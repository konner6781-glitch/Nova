use nova::value::Value;

#[test]
fn duplicate_dictionary_keys_use_the_last_value_everywhere() {
    let duplicate = Value::Dictionary(vec![
        (Value::from("key"), Value::Int(1)),
        (Value::from("key"), Value::Int(2)),
    ]);
    let canonical = Value::Dictionary(vec![(Value::from("key"), Value::Int(2))]);
    assert_eq!(duplicate, canonical);
    assert_eq!(duplicate.to_string(), "{key: 2}");

    let different_final_value = Value::Dictionary(vec![
        (Value::from("key"), Value::Int(2)),
        (Value::from("key"), Value::Int(1)),
    ]);
    assert_ne!(duplicate, different_final_value);
}

#[test]
fn dictionary_equality_is_order_independent_and_recursive() {
    let left = Value::Dictionary(vec![
        (
            Value::Int(1),
            Value::Dictionary(vec![(Value::from("nested"), Value::Bool(true))]),
        ),
        (Value::from("other"), Value::Int(3)),
    ]);
    let right = Value::Dictionary(vec![
        (Value::from("other"), Value::Int(3)),
        (
            Value::Int(1),
            Value::Dictionary(vec![(Value::from("nested"), Value::Bool(true))]),
        ),
    ]);
    assert_eq!(left, right);
    assert_ne!(
        left,
        Value::Dictionary(vec![
            (
                Value::Int(1),
                Value::Dictionary(vec![(Value::from("nested"), Value::Bool(false))])
            ),
            (Value::from("other"), Value::Int(3)),
        ])
    );
}

#[test]
fn float_conversion_rejects_nan_and_infinities() {
    for text in ["NaN", "inf", "-inf"] {
        assert!(Value::from(text).to_float().is_err(), "accepted {text}");
    }
    assert!(Value::Float(f64::NAN).to_float().is_err());
    assert!(Value::Float(f64::INFINITY).to_float().is_err());
    assert!(Value::Float(f64::NAN).to_bool().is_err());
    assert!(
        Value::List(vec![Value::Float(f64::INFINITY)])
            .validate_finite()
            .is_err()
    );
}
