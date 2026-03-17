enum Color {
    Red,
    Green,
    Blue,
}

fn describe(color: Color) -> &'static str {
    match color {
        Color::Red => "warm",
        _ => "cool",
    }
}

fn main() {
    println!("{}", describe(Color::Blue));
}
