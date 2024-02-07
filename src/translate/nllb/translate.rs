use once_cell::sync::Lazy;
use pyo3::{types::PyModule, Py, PyAny, Python};

static TRANSLATE_FN: Lazy<Py<PyAny>> = Lazy::new(|| Python::with_gil(|py| {
    PyModule::from_code(py, include_str!("translate.py"), "translate.py", "translate").unwrap().getattr("translate").unwrap().into()
}));

pub fn translate(s1: &str) -> anyhow::Result<String> {
    Ok(Python::with_gil(|py| TRANSLATE_FN.call1(py, (s1,))?.extract(py))?)
}
