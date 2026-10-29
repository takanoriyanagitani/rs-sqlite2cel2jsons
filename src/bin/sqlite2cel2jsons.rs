use std::io;
use std::process::ExitCode;

use io::Read;

use rs_sqlite2cel2jsons::CelConfig;
use rs_sqlite2cel2jsons::Config;
use rs_sqlite2cel2jsons::SqlConfig;

fn io_envkey2val(key: &'static str) -> impl FnMut() -> Result<String, io::Error> {
    move || std::env::var(key).or_else(|_| Ok(String::default()))
}

fn io_query() -> impl FnMut() -> Result<String, io::Error> {
    io_envkey2val("ENV_SQL")
}

fn io_path2sqlite() -> impl FnMut() -> Result<String, io::Error> {
    io_envkey2val("ENV_SQLITE_DB_FILE_PATH")
}

pub const MAX_PROG_SIZE_DEFAULT: u64 = 1048576;

fn string2u64(s: String) -> Result<u64, io::Error> {
    str::parse(&s).map_err(io::Error::other)
}

fn bind<F, G, T, U, E>(mut iofn: F, pure: G) -> impl FnMut() -> Result<U, E>
where
    F: FnMut() -> Result<T, E>,
    G: Fn(T) -> Result<U, E>,
{
    move || {
        let t: T = iofn()?;
        pure(t)
    }
}

fn io_max_prog_size() -> impl FnMut() -> Result<u64, io::Error> {
    bind(io_envkey2val("ENV_MAX_PROG_SIZE"), string2u64)
}

fn io_program_source() -> impl FnMut() -> Result<String, io::Error> {
    || {
        let limit: u64 = io_max_prog_size()().unwrap_or(MAX_PROG_SIZE_DEFAULT);
        let brdr = io::stdin().lock();
        let mut taken = brdr.take(limit);
        let mut buf: String = String::default();
        taken.read_to_string(&mut buf)?;
        Ok(buf)
    }
}

fn io_celcfg() -> impl FnMut() -> Result<CelConfig, io::Error> {
    || {
        let source: String = io_program_source()()?;
        Ok(source.into())
    }
}

fn io_sqlcfg() -> impl FnMut() -> Result<SqlConfig, io::Error> {
    || {
        let path2sqlite: String = io_path2sqlite()()?;
        let query: String = io_query()()?;
        Ok(SqlConfig { path2sqlite, query })
    }
}

fn io_cfg() -> impl FnMut() -> Result<Config, io::Error> {
    || {
        let cel: CelConfig = io_celcfg()()?;
        let sql: SqlConfig = io_sqlcfg()()?;
        Ok(Config { cel, sql })
    }
}

fn io_main() -> impl FnOnce() -> Result<(), io::Error> {
    || {
        let cfg: Config = io_cfg()()?;
        cfg.sql2rows2cel2jsons2stdout()
    }
}

fn sub() -> Result<(), io::Error> {
    io_main()()
}

fn main() -> ExitCode {
    sub().map(|_| ExitCode::SUCCESS).unwrap_or_else(|e| {
        eprintln!("{e}");

        let cfg: Config = io_cfg()().unwrap_or_default();

        let Config { sql, .. } = cfg;

        let SqlConfig { path2sqlite, query } = sql;

        let limit: u64 = io_max_prog_size()().unwrap_or(MAX_PROG_SIZE_DEFAULT);

        eprintln!("ENV_SQL: {query}");
        eprintln!("ENV_SQLITE_DB_FILE_PATH: {path2sqlite}");
        eprintln!("ENV_MAX_PROG_SIZE: {limit}");

        ExitCode::FAILURE
    })
}
