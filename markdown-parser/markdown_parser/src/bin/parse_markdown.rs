pub fn main() -> Result<(), Box<dyn std::error::Error>>{
    let content = std::fs::read_to_string(std::env::args().nth(1).unwrap())?;

    let html = markdown_parser::markdown_to_html(&content, 0)?;
    std::fs::write("test.html", html)?;

    Ok(())
}
