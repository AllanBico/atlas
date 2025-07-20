fn expand_value(start_val: f64, end_val: f64, step_val: f64) -> Vec<f64> {
    let mut vals = vec![];
    let mut v = start_val;
    while v <= end_val + 1e-8 {
        vals.push(v);
        v += step_val;
    }
    vals
}

fn main() {
    // Test the exact values from the logs
    let test_cases = vec![
        ("adx_period", 10.0, 14.0, 4.0),
        ("adx_range_threshold", 20.0, 30.0, 10.0),
        ("bband_period", 18.0, 22.0, 2.0),
        ("rsi_oversold", 20.0, 30.0, 10.0),
        ("rsi_period", 10.0, 14.0, 4.0),
        ("rsi_smoothing", 3.0, 5.0, 2.0),
    ];
    
    for (name, start, end, step) in test_cases {
        let result = expand_value(start, end, step);
        println!("{}: {:?} ({} values)", name, result, result.len());
    }
    
    // Calculate total combinations
    let total = 2 * 2 * 3 * 2 * 1 * 2 * 2; // Expected values
    println!("\nExpected total combinations: {}", total);
} 