
fn generate_macros() {
    println!("#[allow(unused_macros)]");
    println!("{}", "macro_rules! sq {");

    for row in 0..8 {
        for col in 0..8 {
            let square = row * 8 + col;
            println!("    (\"{}{}\") => {{ {} }};", (b'a' + col) as char, row + 1, square);
        }
    }
    println!("    {}", "($other:expr) => {");
    println!("    {}", "    compile_error!(\"Invalid square coordinate\");");
    println!("    }};");
    println!("}}");
}


#[cfg(test)]
mod tests {
    use crate::util::sq_macro_generator::generate_macros;

    #[test]
    fn test_generate_macros() {
        generate_macros();
    }
}
