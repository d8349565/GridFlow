use excel_workbench::{model::{Cell, ColumnType}, reader::suggest_type};

#[test]
fn early_type_decisions_keep_the_full_sample_eighty_percent_threshold() {
    for size in [1, 2, 4, 5, 6, 20, 99, 100] {
        for matching in 0..=size {
            for date in [false, true] {
                let mut values = vec![Cell::Text("ordinary text".into()); size];
                for value in values.iter_mut().take(matching) {
                    *value = if date { Cell::Date("2026-11-11".into()) } else { Cell::Number(42.) };
                }
                let expected = if matching * 5 >= size * 4 {
                    if date { ColumnType::Date } else { ColumnType::Number }
                } else { ColumnType::Text };
                assert_eq!(suggest_type(&values), expected, "size={size} matching={matching} date={date}");
                values.reverse();
                assert_eq!(suggest_type(&values), expected, "reversed size={size} matching={matching} date={date}");
            }
        }
    }
    assert_eq!(suggest_type(&[]), ColumnType::Text);
    assert_eq!(suggest_type(&vec![Cell::Text("20261111".into()); 100]), ColumnType::Date);
    for value in ["00123", "1234567890123456"] {
        let mut values = vec![Cell::Number(42.); 99];
        values.push(Cell::Text(value.into()));
        assert_eq!(suggest_type(&values), ColumnType::Text);
    }
}
