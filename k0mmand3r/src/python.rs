
// file src/python.rs
use std::collections::HashMap;
use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use crate::KmdLine;  // Replace with actual module path as needed


// PyKmdLine is a Python-friendly version of KmdLine
#[cfg(feature = "lang-python")]
#[pyclass]
pub struct PyKmdLine {
    verb: Option<String>,
    params: Option<HashMap<String, String>>, // Adjust as needed
    content: Option<String>,
}

#[cfg(feature = "lang-python")]
#[pymethods]
impl PyKmdLine {
    #[new]
    pub fn new(verb: Option<String>, params: Option<HashMap<String, String>>, content: Option<String>) -> Self {
        PyKmdLine { verb, params, content }
    }

    #[getter]
    pub fn verb(&self) -> Option<String> {
        self.verb.clone()
    }

    #[getter]
    pub fn params(&self) -> Option<HashMap<String, String>> {
        self.params.clone()
    }

    #[getter]
    pub fn content(&self) -> Option<String> {
        self.content.clone()
    }

    // Additional methods to expose to Python can be added here
}


// Assuming you have a function or method to convert KmdLine to PyKmdLine
// This could be a method in KmdLine or a standalone function
#[cfg(feature = "lang-python")]
fn convert_to_pykmdline(kmdline: KmdLine) -> PyKmdLine {
    // Conversion logic here...
    PyKmdLine {
        verb: kmdline.verb,
        params: kmdline.params.map(|params| {
            params.kvs.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
        }),
        content: kmdline.content,
    }
}


#[cfg(feature = "lang-python")]
#[pyfunction]
pub fn parse_kmd_line(input: String) -> PyResult<PyKmdLine> {
    let mut input_str = &input[..];
    match KmdLine::parse(&mut input_str) {
        Ok(kmdline) => {
            Ok(convert_to_pykmdline(kmdline))
        },
        // Ok(kmdline) => {
        //     // Serialize KmdLine to JSON string
        //     serde_json::to_string(&kmdline)
        //         .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
        //             format!("Serialization error: {:?}", e)
        //         ))
        // },
        Err(e) => {
            Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                format!("Parse error: {:?}", e)
            ))
        }
    }
}




// Don't forget to add this function to your Python module
#[cfg(feature = "lang-python")]
#[pymodule]
pub fn k0mmand3r(py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse_kmd_line, m)?)?;
    Ok(())
}





