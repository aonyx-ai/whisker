enum Color {
    Red,
    Green,
    Blue,
}

fn unused_function() {}

fn main() {
    let color = Color::Red;
    match color {
        Color::Red => {}
        _ => {}
    }
}
