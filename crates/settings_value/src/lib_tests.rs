use super::*;

#[test]
fn duration_round_trip() {
    let d = Duration::from_secs(30);
    let file_val = d.to_file_value();
    assert_eq!(file_val, Value::Number(30.into()));
    let back = Duration::from_file_value(&file_val).unwrap();
    assert_eq!(back, d);
}

#[test]
fn vec_recursive() {
    let v = vec![10u32, 20u32];
    let file_val = v.to_file_value();
    assert_eq!(
        file_val,
        Value::Array(vec![Value::Number(10.into()), Value::Number(20.into())])
    );
    let back = Vec::<u32>::from_file_value(&file_val).unwrap();
    assert_eq!(back, v);
}

#[test]
fn vec_skips_unparseable_elements() {
    // A newer build may have removed an enum variant that an older settings
    // file still references. Unparseable elements are dropped, and the rest of
    // the list survives instead of the whole setting resetting to default.
    let file_val = Value::Array(vec![
        Value::Number(10.into()),
        Value::String("not a number".into()),
        Value::Number(30.into()),
    ]);
    let back = Vec::<u32>::from_file_value(&file_val).unwrap();
    assert_eq!(back, vec![10u32, 30u32]);
}

#[test]
fn vec_rejects_non_array() {
    assert_eq!(Vec::<u32>::from_file_value(&Value::Number(1.into())), None);
}

#[test]
fn option_some() {
    let v: Option<u32> = Some(5);
    let file_val = v.to_file_value();
    assert_eq!(file_val, Value::Number(5.into()));
    let back = Option::<u32>::from_file_value(&file_val).unwrap();
    assert_eq!(back, v);
}

#[test]
fn option_none() {
    let v: Option<u32> = None;
    let file_val = v.to_file_value();
    assert_eq!(file_val, Value::Null);
    let back = Option::<u32>::from_file_value(&file_val).unwrap();
    assert_eq!(back, v);
}

#[test]
fn bool_passthrough() {
    assert_eq!(true.to_file_value(), Value::Bool(true));
    assert_eq!(bool::from_file_value(&Value::Bool(false)), Some(false));
}

#[test]
fn string_passthrough() {
    let s = "hello".to_string();
    let file_val = s.to_file_value();
    assert_eq!(file_val, Value::String("hello".into()));
    assert_eq!(String::from_file_value(&file_val), Some(s));
}

#[test]
fn hashmap_round_trip() {
    let mut m = HashMap::new();
    m.insert("key".to_string(), 42u32);
    let file_val = m.to_file_value();
    let obj = file_val.as_object().unwrap();
    assert_eq!(obj.get("key"), Some(&Value::Number(42.into())));
    let back = HashMap::<String, u32>::from_file_value(&file_val).unwrap();
    assert_eq!(back, m);
}
