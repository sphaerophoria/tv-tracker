const IO_BUFFER_LEN_CONST: usize = 16384;

#[no_mangle]
pub static IO_BUFFER_LEN: usize = IO_BUFFER_LEN_CONST;

#[no_mangle]
pub static mut IO_BUFFER: [u8; IO_BUFFER_LEN_CONST] = [0xaa; IO_BUFFER_LEN_CONST];

#[no_mangle]
pub extern fn parse_markdown(len: usize, heading_offset: usize) -> usize {
    let s = unsafe {
        let s = std::str::from_utf8_unchecked(&IO_BUFFER[0..len]);
        s.to_string()
    };

    let Ok(output_html) = markdown_parser::markdown_to_html(&s, heading_offset) else { return 0 };

    unsafe {
        IO_BUFFER[0..output_html.len()].copy_from_slice(output_html.as_bytes());
    }
    output_html.len()
}
