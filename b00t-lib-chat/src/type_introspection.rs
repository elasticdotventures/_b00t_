//! Runtime type descriptors for b00t chat/domain surfaces.
//!
//! Rust does not expose structural reflection at runtime. This module keeps the
//! contract explicit: types are introspectable through a trait, and repetitive
//! descriptor metadata is inferred from macro input instead of handwritten
//! functions.

use std::collections::BTreeMap;

pub type TypeMetadata = BTreeMap<String, String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TypeShape {
    Struct,
    Enum,
    Trait,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FieldDescriptor {
    pub name: &'static str,
    pub rust_type: &'static str,
    pub classifier: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VariantDescriptor {
    pub name: &'static str,
    pub classifier: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeDescriptor {
    pub rust_type: &'static str,
    pub shape: TypeShape,
    pub classifier: &'static str,
    pub fields: Vec<FieldDescriptor>,
    pub variants: Vec<VariantDescriptor>,
    pub metadata: TypeMetadata,
}

pub trait TypeIntrospection {
    fn type_descriptor() -> TypeDescriptor;
}

#[macro_export]
macro_rules! impl_type_introspection {
    (
        struct $target:ty {
            classifier: $classifier:expr,
            fields: [$( $field:ident : $field_ty:ty => $field_classifier:expr ),* $(,)?] $(,)?
        }
    ) => {
        impl $crate::type_introspection::TypeIntrospection for $target {
            fn type_descriptor() -> $crate::type_introspection::TypeDescriptor {
                $crate::type_introspection::TypeDescriptor {
                    rust_type: std::any::type_name::<$target>(),
                    shape: $crate::type_introspection::TypeShape::Struct,
                    classifier: $classifier,
                    fields: vec![
                        $(
                            $crate::type_introspection::FieldDescriptor {
                                name: stringify!($field),
                                rust_type: std::any::type_name::<$field_ty>(),
                                classifier: $field_classifier,
                            }
                        ),*
                    ],
                    variants: Vec::new(),
                    metadata: $crate::type_introspection::TypeMetadata::new(),
                }
            }
        }
    };
    (
        enum $target:ty {
            classifier: $classifier:expr,
            variants: [$( $variant:ident => $variant_classifier:expr ),* $(,)?] $(,)?
        }
    ) => {
        impl $crate::type_introspection::TypeIntrospection for $target {
            fn type_descriptor() -> $crate::type_introspection::TypeDescriptor {
                $crate::type_introspection::TypeDescriptor {
                    rust_type: std::any::type_name::<$target>(),
                    shape: $crate::type_introspection::TypeShape::Enum,
                    classifier: $classifier,
                    fields: Vec::new(),
                    variants: vec![
                        $(
                            $crate::type_introspection::VariantDescriptor {
                                name: stringify!($variant),
                                classifier: $variant_classifier,
                            }
                        ),*
                    ],
                    metadata: $crate::type_introspection::TypeMetadata::new(),
                }
            }
        }
    };
    (
        trait $target:ty {
            classifier: $classifier:expr $(,)?
        }
    ) => {
        impl $crate::type_introspection::TypeIntrospection for $target {
            fn type_descriptor() -> $crate::type_introspection::TypeDescriptor {
                $crate::type_introspection::TypeDescriptor {
                    rust_type: std::any::type_name::<$target>(),
                    shape: $crate::type_introspection::TypeShape::Trait,
                    classifier: $classifier,
                    fields: Vec::new(),
                    variants: Vec::new(),
                    metadata: $crate::type_introspection::TypeMetadata::new(),
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CellAddress, FlashSheet, LogicalAddress, flash_sheet_type_descriptors,
        state_machine_type_descriptors,
    };
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct ExpectedRegistry {
        count: usize,
        classifiers: Vec<String>,
    }

    #[derive(Debug, Deserialize)]
    struct ExpectedFixture {
        flash_sheet: ExpectedRegistry,
        state_machine: ExpectedRegistry,
    }

    fn expected_fixture() -> ExpectedFixture {
        serde_json::from_str(include_str!(
            "../tests/fixtures/type_introspection_expected.json"
        ))
        .unwrap()
    }

    #[test]
    fn registries_match_expected_type_classifiers() {
        let expected = expected_fixture();
        let flash_sheet = flash_sheet_type_descriptors();
        let state_machine = state_machine_type_descriptors();

        assert_eq!(flash_sheet.len(), expected.flash_sheet.count);
        assert_eq!(
            flash_sheet
                .iter()
                .map(|descriptor| descriptor.classifier.to_string())
                .collect::<Vec<_>>(),
            expected.flash_sheet.classifiers
        );
        assert_eq!(state_machine.len(), expected.state_machine.count);
        assert_eq!(
            state_machine
                .iter()
                .map(|descriptor| descriptor.classifier.to_string())
                .collect::<Vec<_>>(),
            expected.state_machine.classifiers
        );
    }

    #[test]
    fn concrete_types_are_introspectable_by_trait() {
        let sheet = <FlashSheet as TypeIntrospection>::type_descriptor();
        let cell_address = <CellAddress as TypeIntrospection>::type_descriptor();
        let logical_address = <LogicalAddress as TypeIntrospection>::type_descriptor();

        assert_eq!(sheet.shape, TypeShape::Struct);
        assert_eq!(sheet.classifier, "flash_sheet.sheet");
        assert!(sheet.fields.iter().any(|field| field.name == "cells"));
        assert_eq!(cell_address.fields.len(), 2);
        assert_eq!(logical_address.fields[0].name, "graph_path");
    }
}
