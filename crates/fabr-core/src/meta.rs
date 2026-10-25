use crate::error::FabrError;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use toml_edit::{DocumentMut, Item, Value};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MetaValue {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
}

pub type Meta = IndexMap<String, MetaValue>;

fn to_meta(v: &Value) -> Option<MetaValue> {
    match v {
        Value::String(s) => Some(MetaValue::Str(s.value().clone())),
        Value::Integer(i) => Some(MetaValue::Int(*i.value())),
        Value::Float(f) => Some(MetaValue::Float(*f.value())),
        Value::Boolean(b) => Some(MetaValue::Bool(*b.value())),
        _ => None,
    }
}

fn to_value(v: &MetaValue) -> Value {
    match v {
        MetaValue::Str(s) => Value::from(s.as_str()),
        MetaValue::Int(i) => Value::from(*i),
        MetaValue::Float(f) => Value::from(*f),
        MetaValue::Bool(b) => Value::from(*b),
    }
}

pub fn parse_meta(text: &str) -> Result<Meta, String> {
    let doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| e.to_string().trim().to_string())?;
    Ok(doc.as_table().iter().filter_map(|(k, item)| Some((k.to_string(), to_meta(item.as_value()?)?))).collect())
}

pub fn render_meta(old: &str, meta: &Meta) -> Result<String, FabrError> {
    let mut doc: DocumentMut =
        old.parse().map_err(|_| FabrError::Unprocessable("meta.toml does not parse; fix it by hand before editing here".into()))?;
    let root = doc.as_table_mut();
    let stale: Vec<String> = root
        .iter()
        .filter(|(k, item)| item.as_value().and_then(to_meta).is_some() && !meta.contains_key(*k))
        .map(|(k, _)| k.to_string())
        .collect();
    for k in stale {
        root.remove(&k);
    }
    for (k, v) in meta {
        match root.get_mut(k) {
            Some(Item::Value(old_v)) => {
                if to_meta(old_v).as_ref() == Some(v) {
                    continue;
                }
                let mut new_v = to_value(v);
                *new_v.decor_mut() = old_v.decor().clone();
                *old_v = new_v;
            }
            Some(Item::None) | None => {
                root.insert(k, Item::Value(to_value(v)));
            }
            Some(_) => {
                return Err(FabrError::BadRequest(format!("meta {k}: is a table in meta.toml and cannot be set here")));
            }
        }
    }
    Ok(doc.to_string())
}

pub const COLOR_NAMES: [&str; 9] = ["blue", "green", "indigo", "orange", "pink", "purple", "red", "teal", "yellow"];

pub fn color_ok(v: &str) -> bool {
    COLOR_NAMES.contains(&v) || (v.len() == 7 && v.starts_with('#') && v[1..].bytes().all(|c| c.is_ascii_hexdigit()))
}

pub fn color_warning(meta: &Meta) -> Option<String> {
    match meta.get("color")? {
        MetaValue::Str(s) if color_ok(s) => None,
        v => Some(format!("color {} is not a tint name or #RRGGBB; ignored", shown(v))),
    }
}

pub(crate) fn shown(v: &MetaValue) -> String {
    match v {
        MetaValue::Str(s) => format!("{s:?}"),
        MetaValue::Int(i) => i.to_string(),
        MetaValue::Float(f) => f.to_string(),
        MetaValue::Bool(b) => b.to_string(),
    }
}

fn key_ok(k: &str) -> bool {
    !k.is_empty() && k.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}

pub fn validate_meta(meta: Meta) -> Result<Meta, FabrError> {
    for (k, v) in &meta {
        if !key_ok(k) {
            return Err(FabrError::BadRequest(format!("bad meta key {k:?}")));
        }
        if let MetaValue::Float(f) = v
            && !f.is_finite()
        {
            return Err(FabrError::BadRequest(format!("meta {k}: numbers must be finite")));
        }
    }
    if let Some(d) = meta.get("show_depth")
        && !matches!(d, MetaValue::Int(n) if *n >= 0)
    {
        return Err(FabrError::BadRequest("show_depth must be an integer >= 0".into()));
    }
    if let Some(h) = meta.get("hidden")
        && !matches!(h, MetaValue::Bool(_))
    {
        return Err(FabrError::BadRequest("hidden must be true or false".into()));
    }
    if let Some(c) = meta.get("color")
        && !matches!(c, MetaValue::Str(s) if color_ok(s))
    {
        return Err(FabrError::BadRequest(format!("color must be one of {} or #RRGGBB (got {})", COLOR_NAMES.join(" "), shown(c))));
    }
    Ok(meta)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(pairs: &[(&str, MetaValue)]) -> Meta {
        pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    #[test]
    fn render_keeps_tables() {
        let old = "name = \"x\"\n\n[extra]\nfoo = 1\n";
        let meta = m(&[("name", MetaValue::Str("a \"q\"".into())), ("show_depth", MetaValue::Int(2)), ("hidden", MetaValue::Bool(true))]);
        let out = render_meta(old, &meta).unwrap();
        let parsed = parse_meta(&out).unwrap();
        assert_eq!(parsed, meta);
        let doc: DocumentMut = out.parse().unwrap();
        assert_eq!(doc["extra"]["foo"].as_integer(), Some(1));
        assert!(out.find("hidden").unwrap() < out.find("[extra]").unwrap());
    }

    #[test]
    fn render_keeps_comments_and_order() {
        let old = "# top comment\nname = \"x\"  # the name\nkind = \"area\"\ngone = 1\n\n# about extra\n[extra]\nfoo = [1, 2]\n[extra.deep]\nbar = true\n";
        let meta = m(&[("name", MetaValue::Str("y".into())), ("kind", MetaValue::Str("area".into())), ("new", MetaValue::Float(1.5))]);
        let out = render_meta(old, &meta).unwrap();
        assert!(out.starts_with("# top comment\nname = \"y\"  # the name\nkind = \"area\"\n"), "{out}");
        assert!(!out.contains("gone"));
        assert!(out.contains("# about extra\n[extra]\nfoo = [1, 2]\n[extra.deep]\nbar = true\n"), "{out}");
        assert_eq!(parse_meta(&out).unwrap(), meta);
        assert_eq!(render_meta(old, &parse_meta(old).unwrap()).unwrap(), old);
    }

    #[test]
    fn render_refuses_bad_old_and_table_keys() {
        assert_eq!(render_meta("name = ", &Meta::new()).unwrap_err().code(), 422);
        let e = render_meta("[extra]\na=1\n", &m(&[("extra", MetaValue::Int(1))])).unwrap_err();
        assert_eq!(e.code(), 400);
        assert_eq!(render_meta("", &m(&[("name", MetaValue::Str("r".into()))])).unwrap(), "name = \"r\"\n");
    }

    #[test]
    fn parse_skips_nested() {
        let meta = parse_meta("a = 1\nb = [1]\nc = { x = 1 }\nd = 1979-05-27\ne = 2.5\n[t]\nf = 1\n").unwrap();
        assert_eq!(meta, m(&[("a", MetaValue::Int(1)), ("e", MetaValue::Float(2.5))]));
        assert!(parse_meta("= nope").is_err());
    }

    #[test]
    fn validation() {
        let bad: Vec<Meta> = vec![
            m(&[("show_depth", MetaValue::Int(-1))]),
            m(&[("show_depth", MetaValue::Str("2".into()))]),
            m(&[("show_depth", MetaValue::Bool(true))]),
            m(&[("hidden", MetaValue::Str("yes".into()))]),
            m(&[("bad key", MetaValue::Int(1))]),
        ];
        for b in bad {
            assert!(validate_meta(b.clone()).is_err(), "{b:?}");
        }
        for j in [r#"{"x":[1]}"#, r#"{"x":{"a":1}}"#, r#"{"x":null}"#] {
            assert!(serde_json::from_str::<Meta>(j).is_err(), "{j}");
        }
        let ok: Meta = serde_json::from_str(r#"{"name":"n","show_depth":2,"hidden":false,"w":1.5}"#).unwrap();
        assert_eq!(ok["show_depth"], MetaValue::Int(2));
        assert_eq!(ok["w"], MetaValue::Float(1.5));
        assert!(validate_meta(ok).is_ok());
    }

    #[test]
    fn color_validation() {
        for good in ["blue", "green", "indigo", "orange", "pink", "purple", "red", "teal", "yellow", "#00aaFF", "#000000"] {
            assert!(validate_meta(m(&[("color", MetaValue::Str(good.into()))])).is_ok(), "{good}");
            assert_eq!(color_warning(&m(&[("color", MetaValue::Str(good.into()))])), None);
        }
        let junk = [
            MetaValue::Str("Blue".into()),
            MetaValue::Str("".into()),
            MetaValue::Str("#fff".into()),
            MetaValue::Str("#12345g".into()),
            MetaValue::Str("#1234567".into()),
            MetaValue::Str("123456".into()),
            MetaValue::Str(" red".into()),
            MetaValue::Str("magenta".into()),
            MetaValue::Str("rgb(1,2,3)".into()),
            MetaValue::Int(1),
            MetaValue::Bool(true),
        ];
        for j in junk {
            let e = validate_meta(m(&[("color", j.clone())])).unwrap_err();
            assert_eq!(e.code(), 400, "{j:?}");
            assert!(color_warning(&m(&[("color", j.clone())])).is_some(), "{j:?}");
        }
        assert_eq!(color_warning(&Meta::new()), None);
    }
}
