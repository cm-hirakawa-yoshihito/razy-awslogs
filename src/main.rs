use failure::Fail;

mod app;
mod cmd;
mod errors;

fn handle_error(e: errors::Error) {
    let prefix = "error";
    match e.cause() {
        Some(fail) => {
            eprintln!("{}: {}: cause='{}'", prefix, e, fail);
            if let Some(backtrace) = fail.backtrace() {
                eprint!("{}", backtrace);
            }
        }
        None => eprintln!("{}: {}", prefix, e),
    };
}

fn main() {
    env_logger::init();

    match app::main() {
        Ok(()) => (),
        Err(e) => handle_error(e),
    }
}
