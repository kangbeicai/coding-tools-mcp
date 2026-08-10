fn main() {
    if let Err(error) = coding_tools_mcp::headless::run_from_env() {
        eprintln!("coding-tools: {error}");
        std::process::exit(1);
    }
}
