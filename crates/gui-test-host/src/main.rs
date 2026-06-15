fn main() {
    let mut duration_ms = 1000;
    let mut width = 400;
    let mut height = 300;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--duration-ms" => {
                duration_ms = args
                    .next()
                    .expect("--duration-ms requires a value")
                    .parse()
                    .expect("duration must be a number");
            }
            "--width" => {
                width = args
                    .next()
                    .expect("--width requires a value")
                    .parse()
                    .expect("width must be a number");
            }
            "--height" => {
                height = args
                    .next()
                    .expect("--height requires a value")
                    .parse()
                    .expect("height must be a number");
            }
            other => eprintln!("warning: unknown argument {other}"),
        }
    }

    gui_test_host::run_test_host(duration_ms, width, height);
}
