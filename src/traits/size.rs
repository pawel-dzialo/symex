use anyhow::anyhow;
use llvm_ir::types::{FPType, NamedStructDef, Type, TypeRef};

use super::Size;
use crate::{
    memory::to_bytes,
    project::Project,
    vm::{Result, State, VMError},
    BV,
};

impl Size for Type {
    fn size(&self, project: &Project) -> Option<u64> {
        size_in_bits(self, project)
    }

    fn size_in_bytes(&self, project: &Project) -> Result<Option<u64>> {
        self.size(project)
            .map(to_bytes)
            .transpose()
            .map_err(|err| VMError::MemoryError(err))
    }

    fn inner_ty(&self, project: &Project) -> Option<TypeRef> {
        get_inner_type(self, project)
    }
}

impl Size for TypeRef {
    fn size(&self, project: &Project) -> Option<u64> {
        self.as_ref().size(project)
    }

    fn size_in_bytes(&self, project: &Project) -> Result<Option<u64>> {
        self.as_ref().size_in_bytes(project)
    }

    fn inner_ty(&self, project: &Project) -> Option<TypeRef> {
        self.as_ref().inner_ty(project)
    }
}

/// Calculates the size of the type in bits.
pub fn size_in_bits(ty: &Type, project: &Project) -> Option<u64> {
    use Type::*;

    match ty {
        VoidType => Some(0),
        IntegerType { bits } => Some(*bits as u64),
        PointerType { .. } => Some(project.ptr_size),
        FPType(fp_ty) => Some(fp_size_in_bits(fp_ty)),
        VectorType {
            element_type,
            num_elements,
            ..
        } => size_in_bits(element_type, project).map(|size| *num_elements as u64 * size),
        ArrayType {
            element_type,
            num_elements,
        } => size_in_bits(element_type, project).map(|size| {
            println!(
                "ARRAY TYPE: element_size: {}, num_elements: {}, total: {}",
                size,
                *num_elements,
                size * *num_elements as u64
            );
            *num_elements as u64 * size
        }),
        StructType { element_types, .. } => element_types
            .iter()
            .map(|ty| size_in_bits(ty, project))
            .sum(),
        NamedStructType { name } => match project.get_named_struct(name)? {
            NamedStructDef::Opaque => None,
            NamedStructDef::Defined(ty) => size_in_bits(ty, project),
        },
        FuncType { .. } => todo!(),
        X86_MMXType => todo!(),
        X86_AMXType => todo!(),
        MetadataType => todo!(),
        LabelType => todo!(),
        TokenType => todo!(),
    }
}

/// Returns the size of a floating point type.
pub fn fp_size_in_bits(ty: &FPType) -> u64 {
    match ty {
        FPType::Half => 16,
        FPType::BFloat => 16,
        FPType::Single => 32,
        FPType::Double => 64,
        FPType::FP128 => 128,
        FPType::X86_FP80 => 80,
        FPType::PPC_FP128 => 128,
    }
}

// pub fn get_offset_bv(ty: &Type, index: &BV, state: &mut State<'_>) -> Result<(BV, TypeRef)> {
//     use Type::*;

//     match ty {
//         PointerType {
//             pointee_type: inner_ty,
//             ..
//         }
//         // LLVM docs:
//         // Q: Can GEP index into vector elements?
//         // A: This hasn’t always been forcefully disallowed, though it’s not
//         //    recommended. It leads to awkward special cases in the optimizers, 
//         //    and fundamental inconsistency in the IR. In the future, it will
//         //    probably be outright disallowed.
//         | VectorType {
//             element_type: inner_ty,
//             ..
//         }
//         | ArrayType {
//             element_type: inner_ty,
//             ..
//         } => {
//             let size = inner_ty.size(&state.project).unwrap();
//             let size = state.solver.bv_from_u64(size, index.len());
//             Ok((size.mul(index), inner_ty.clone()))
//         }

//         // Not supported for non-constant indexes.
//         NamedStructType { .. } => todo!(),
//         StructType { .. } => todo!(),

//         // Should not happend for these instructions.
//         VoidType => todo!(),
//         IntegerType { .. } => todo!(),
//         FPType(_) => todo!(),
//         FuncType { .. } => todo!(),
//         X86_MMXType => todo!(),
//         X86_AMXType => todo!(),
//         MetadataType => todo!(),
//         LabelType => todo!(),
//         TokenType => todo!(),
//     }
// }

/// Calculate the offset in bytes from a concrete index.
///
/// Note that the conversion from bits to bytes is performed after the offset has been calculated.
/// This means that is some of the offsets are not byte divisible, they may still work, as long as
/// the final result is byte divisible. However, this is just an error in the IR then which is
/// a compiler error and not a program error.
pub fn get_byte_offset_concrete(
    ty: &Type,
    index: u64,
    project: &Project,
) -> Result<(u64, TypeRef)> {
    let (offset_in_bits, ty) = get_bit_offset_concrete(ty, index, project)?;
    let offset = to_bytes(offset_in_bits)?;
    Ok((offset, ty))
}

/// Calculate the offset in bits from a concrete index.
pub fn get_bit_offset_concrete(ty: &Type, index: u64, project: &Project) -> Result<(u64, TypeRef)> {
    use Type::*;

    match ty {
        // Offsets for pointers, vector and arrays are similar in that the offset is just the size
        // of the base type times the index.
        //
        // Note that in the LLVM docs for GEP vector indexing may be disallowed in the future,
        // thus it should not really happen.
        #[rustfmt::skip]
        PointerType { pointee_type: inner_ty, .. }
        | VectorType { element_type: inner_ty, .. }
        | ArrayType { element_type: inner_ty, .. } => {
            size_in_bits(ty, project)
                .map(|size| (size * index, inner_ty.clone()))
                .ok_or_else(|| anyhow!("Cannot take size of type").into())
        },

        // For structs we have to collect the size of all the members until the index.
        //
        // TODO: How are non packed structs handled?
        StructType {
            element_types,
            is_packed: _,
        } => {
            let offset = element_types
                .iter()
                .take(index as usize)
                .map(|ty| size_in_bits(ty, project).ok_or_else(|| anyhow!("Oops").into()))
                .sum::<Result<u64>>()?;

            let inner_ty = element_types
                .get(index as usize)
                .cloned()
                .ok_or_else(|| VMError::MalformedInstruction)?;

            Ok((offset, inner_ty))
        }

        // For named structs we just have to look these up in the project.
        //
        // If the result in an Opaque struct it cannot proceeed.
        NamedStructType { name } => match project.get_named_struct(name).unwrap() {
            NamedStructDef::Opaque => todo!(),
            NamedStructDef::Defined(ty) => get_bit_offset_concrete(ty, index, project),
        },

        // TODO
        VoidType => todo!(),
        IntegerType { .. } => todo!(),
        FPType(_) => todo!(),
        FuncType { .. } => todo!(),
        X86_MMXType => todo!(),
        X86_AMXType => todo!(),
        MetadataType => todo!(),
        LabelType => todo!(),
        TokenType => todo!(),
    }
}

/// Get the byte offset with a symbol as index.
/// 
/// This checks that each offset is byte divisible.
pub fn get_byte_offset_symbol(ty: &Type, index: &BV, state: &mut State<'_>) -> Result<(BV, TypeRef)> {
    use Type::*;

    match ty {
        // Offsets for pointers, vector and arrays are similar in that the offset is just the size
        // of the base type times the index.
        #[rustfmt::skip]
        PointerType { pointee_type: inner_ty, .. }
        | VectorType { element_type: inner_ty, .. }
        | ArrayType { element_type: inner_ty, .. } => {
            let size = size_in_bits(ty, &state.project)
                .ok_or_else(|| VMError::Other(anyhow!("Cannot take size of type")))?;
            
            let size = to_bytes(size)?;
            let size = state.solver.bv_from_u64(size, index.len());
            Ok((size.mul(&index), inner_ty.clone()))
        },
        
        // Not supported for non-constant indexes.
        //
        // With a symbol as an index we cannot index into structs, not without having to try
        // solutions and fork the state. So these are not supported.
        StructType { .. } | NamedStructType { .. } => panic!("Symbolic struct index is not supported"),

        // TODO
        VoidType => todo!(),
        IntegerType { .. } => todo!(),
        FPType(_) => todo!(),
        FuncType { .. } => todo!(),
        X86_MMXType => todo!(),
        X86_AMXType => todo!(),
        MetadataType => todo!(),
        LabelType => todo!(),
        TokenType => todo!(),
    }
}

/// Get the offset in bits with a symbol as index.
pub fn get_bit_offset_symbol(ty: &Type, index: &BV, state: &mut State<'_>) -> Result<(BV, TypeRef)> {
    use Type::*;

    match ty {
        // Offsets for pointers, vector and arrays are similar in that the offset is just the size
        // of the base type times the index.
        #[rustfmt::skip]
        PointerType { pointee_type: inner_ty, .. }
        | VectorType { element_type: inner_ty, .. }
        | ArrayType { element_type: inner_ty, .. } => {
            let size = size_in_bits(ty, &state.project)
                .ok_or_else(|| VMError::Other(anyhow!("Cannot take size of type")))?;
            
            let size = state.solver.bv_from_u64(size, index.len());
            Ok((size.mul(&index), inner_ty.clone()))
        },

        
        // Not supported for non-constant indexes.
        //
        // With a symbol as an index we cannot index into structs, not without having to try
        // solutions and fork the state. So these are not supported.
        StructType { .. } | NamedStructType { .. } => panic!("Symbolic struct index is not supported"),

        // TODO
        VoidType => todo!(),
        IntegerType { .. } => todo!(),
        FPType(_) => todo!(),
        FuncType { .. } => todo!(),
        X86_MMXType => todo!(),
        X86_AMXType => todo!(),
        MetadataType => todo!(),
        LabelType => todo!(),
        TokenType => todo!(),
    }
}

// /// Calculate the offset from a concrete index.
// ///
// /// TODO: LLVM docs note that it is allowed for negative indices.
// pub fn get_offset_concrete(ty: &Type, index: u64, project: &Project) -> Result<(u64, TypeRef)> {
//     use Type::*;

//     match ty {
//         PointerType {
//             pointee_type: inner_ty,
//             ..
//         }
//         | VectorType {
//             element_type: inner_ty,
//             ..
//         }
//         | ArrayType {
//             element_type: inner_ty,
//             ..
//         } => {
//             let size = inner_ty.size(project).unwrap();
//             Ok((size * index, inner_ty.clone()))
//         }
//         // TODO: Structs have an `is_packed`, this should probably affect the results...
//         StructType { element_types, .. } => {
//             let offset = element_types
//                 .iter()
//                 .take(index as usize)
//                 .map(|ty| size_in_bits(ty, project).ok_or_else(|| VMError::Other(anyhow!("Oops"))))
//                 .sum::<Result<u64>>()?;

//             let ty = element_types.get(index as usize).cloned().unwrap();

//             Ok((offset, ty))
//         }
//         NamedStructType { name } => match project.get_named_struct(name).unwrap() {
//             NamedStructDef::Opaque => todo!(),
//             NamedStructDef::Defined(ty) => get_offset_concrete(ty, index, project),
//         },

//         // Should not happend for these instructions.
//         VoidType => todo!(),
//         IntegerType { .. } => todo!(),
//         FPType(_) => todo!(),
//         FuncType { .. } => todo!(),
//         X86_MMXType => todo!(),
//         X86_AMXType => todo!(),
//         MetadataType => todo!(),
//         LabelType => todo!(),
//         TokenType => todo!(),
//     }
// }

/// Returns the inner type of the passed [`Type`].
///
/// If an inner type exists, it is returned in `Some`, otherwise `None` is
/// returned.
pub fn get_inner_type(ty: &Type, project: &Project) -> Option<TypeRef> {
    use Type::*;

    match ty {
        VoidType => None,
        IntegerType { .. } => None,
        PointerType { pointee_type, .. } => Some(pointee_type.clone()),
        FPType(_) => todo!(),
        FuncType { .. } => todo!(),
        VectorType { .. } => todo!(),
        ArrayType { element_type, .. } => Some(element_type.clone()),
        StructType { .. } => todo!(),
        NamedStructType { name } => match project.get_named_struct(name).unwrap() {
            NamedStructDef::Opaque => todo!(),
            NamedStructDef::Defined(ty) => get_inner_type(ty, project),
        },
        X86_MMXType => todo!(),
        X86_AMXType => todo!(),
        MetadataType => todo!(),
        LabelType => todo!(),
        TokenType => todo!(),
    }
}
