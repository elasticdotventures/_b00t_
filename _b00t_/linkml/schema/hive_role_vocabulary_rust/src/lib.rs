#![allow(non_camel_case_types)]

#[cfg(feature = "serde")]
mod serde_utils;
pub mod poly;
pub mod poly_containers;
#[cfg(feature = "stubgen")]
pub mod stub_utils;

#[cfg(feature = "serde")]
use serde_yml as _ ;
#[cfg(feature = "pyo3")]
use pyo3::{FromPyObject,prelude::*};
#[cfg(feature = "stubgen")]
use pyo3_stub_gen::{define_stub_info_gatherer,derive::gen_stub_pyclass,derive::gen_stub_pymethods};
#[cfg(feature = "serde")]
use serde::{Deserialize,Serialize,de::IntoDeserializer};
use serde_value::Value;
#[cfg(feature = "serde")]
use serde_path_to_error;
use std::collections::HashMap;
use std::collections::BTreeMap;

// Types

pub type string = String;
pub type integer = String;
pub type boolean = String;
pub type float = f64;
pub type double = f64;
pub type decimal = String;
pub type time = String;
pub type date = String;
pub type datetime = String;
pub type date_or_datetime = String;
pub type uriorcurie = String;
pub type curie = String;
pub type uri = String;
pub type ncname = String;
pub type objectidentifier = String;
pub type nodeidentifier = String;
pub type jsonpointer = String;
pub type jsonpath = String;
pub type sparqlpath = String;

// Slots


// Enums

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum HiveRole {
#[cfg_attr(feature = "serde", serde(rename = "worker"))]
    Worker,
#[cfg_attr(feature = "serde", serde(rename = "executive"))]
    Executive,
#[cfg_attr(feature = "serde", serde(rename = "operator"))]
    Operator,
#[cfg_attr(feature = "serde", serde(rename = "specialist"))]
    Specialist,
}

impl core::fmt::Display for HiveRole {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            HiveRole::Worker => f.write_str("worker"),
            HiveRole::Executive => f.write_str("executive"),
            HiveRole::Operator => f.write_str("operator"),
            HiveRole::Specialist => f.write_str("specialist"),
        }
    }
}

#[cfg(feature = "pyo3")]
impl<'py> IntoPyObject<'py> for HiveRole {
    type Target = PyAny;
    type Output = Bound<'py, Self::Target>;
    type Error = PyErr;
    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        let s: &str = match self {
            HiveRole::Worker => "worker",
            HiveRole::Executive => "executive",
            HiveRole::Operator => "operator",
            HiveRole::Specialist => "specialist",
        };
        Ok(pyo3::types::PyString::new(py, s).into_any())
    }
}

#[cfg(feature = "pyo3")]
impl<'py> FromPyObject<'py> for HiveRole {
    fn extract_bound(ob: &pyo3::Bound<'py, pyo3::types::PyAny>) -> pyo3::PyResult<Self> {
        if let Ok(s) = ob.extract::<&str>() {
            match s {
                "worker" | "Worker" => Ok(HiveRole::Worker),
                "executive" | "Executive" => Ok(HiveRole::Executive),
                "operator" | "Operator" => Ok(HiveRole::Operator),
                "specialist" | "Specialist" => Ok(HiveRole::Specialist),
                _ => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    format!("invalid value for HiveRole: {}", s),
                )),
            }
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                concat!("expected str for ", stringify!(HiveRole)),
            ))
        }
    }
}

#[cfg(feature = "stubgen")]
impl ::pyo3_stub_gen::PyStubType for HiveRole {
    fn type_output() -> ::pyo3_stub_gen::TypeInfo {
        ::pyo3_stub_gen::TypeInfo::with_module(
            "typing.Literal['worker', 'executive', 'operator', 'specialist']",
            "typing".into(),
        )
    }
}

// Classes




#[cfg(feature = "stubgen")]
define_stub_info_gatherer!(stub_info);
