use crate::opts::Opts;
use crate::set_exec;

use std::path::Path;
use std::path::PathBuf;
use std::string;

use colored::Colorize;
use ex::fs;
use ex::io;
use handlebars::Handlebars;
use serde::Serialize;
use snafu::ResultExt;
use snafu::Snafu;


#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("solve script template is not valid UTF-8: {}", source))]
    Utf8Error { source: string::FromUtf8Error },

    #[snafu(display("error writing solve script template: {}", source))]
    WriteError { source: io::Error },

    #[snafu(display("error reading solve script template: {}", source))]
    ReadError { source: io::Error },

    #[snafu(display("error initializing template: {}", source))]
    TmplError { source: handlebars::TemplateError },

    #[snafu(display("error rendering solve script template: {}", source))]
    RenderError { source: handlebars::RenderError },

    #[snafu(display("error setting solve script template executable: {}", source))]
    SetExecError { source: io::Error },
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Serialize)]
struct Bindings {
    exe: String,
    libc: String,
}

/// Make pwntools script that binds the (binary, libc, linker) to `ELF`
/// variables
fn _make_bindings(opts: &Opts) -> String {
    // Helper to make one binding line
    let bind_line = |name: &str, opt_path: &Option<PathBuf>| -> Option<String> {
        opt_path
            .as_ref()
            .map(|path| format!("{} = ELF(\"{}\")", name, path.display(),))
    };

    // Create bindings and join them with newlines
    [
        bind_line(&opts.template_bin_name, &opts.bin),
        bind_line(&opts.template_libc_name, &opts.libc),
        bind_line(&opts.template_ld_name, &opts.ld),
    ]
    .iter()
    .filter_map(|x| x.as_ref())
    .cloned()
    .collect::<Vec<String>>()
    .join("\n")
}

/// Make arguments to pwntools `process()` function
fn _make_proc_args(opts: &Opts) -> String {
    let args = if opts.ld.is_some() {
        format!(
            "{}.path, {}.path",
            opts.template_ld_name, opts.template_bin_name
        )
    } else {
        format!("{}.path", opts.template_bin_name)
    };

    let env = if opts.libc.is_some() {
        format!(", env={{\"LD_PRELOAD\": {}.path}}", opts.template_libc_name)
    } else {
        "".to_string()
    };

    format!("[{}]{}", args, env)
}

/// Fill in template pwntools solve script with (binary, libc, linker) paths
fn make_stub(opts: &Opts) -> Result<String> {
    let templ = match &opts.template_path {
        Some(path) => {
            let data = fs::read(path).context(ReadError)?;
            String::from_utf8(data).context(Utf8Error)?
        }
        None => include_str!("template.py").to_string(),
    };

    let exe = match opts.bin.as_ref() {
        Some(b) => b.to_str().unwrap().to_string(),
        None => "".to_string(),
    };

    let libc = match opts.libc.as_ref() {
        Some(l) => l.to_str().unwrap().to_string(),
        None => "".to_string(),
    };

    let mut handlebars = Handlebars::new();
    handlebars.register_template_string("solve", templ.to_owned()).context(TmplError)?;

    let mapping = Bindings { exe, libc };

    Ok(handlebars.render("solve", &mapping).context(RenderError)?)
}

/// Write script produced with `make_stub()` to `solve.py` in the
/// specified directory, unless a `solve.py` already exists
pub fn write_stub(opts: &Opts) -> Result<()> {
    let stub = make_stub(opts)?;
    let path = Path::new("solve.py");
    if !path.exists() {
        println!("{}", "writing solve.py stub".cyan().bold());
        fs::write(&path, stub).context(WriteError)?;
        set_exec(&path).context(SetExecError)?;
    }
    Ok(())
}
