use std::io;
use std::sync::Arc;

use io::BufWriter;
use io::Write;

use rusqlite::Connection;
use rusqlite::Statement;

use rusqlite::Row;
use rusqlite::Rows;

use rusqlite::types::ValueRef;

use cel::Value as CelVal;
use serde_json::Value as JsonVal;

use cel::Context;
use cel::Env;
use cel::Program;

pub fn cval2jval(cval: &CelVal) -> Option<JsonVal> {
    cval.json().ok()
}

pub fn ctx2jval(p: &Program, ctx: &Context) -> Result<JsonVal, io::Error> {
    let cval: CelVal = p.execute(ctx).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unable to execute: {e}"),
        )
    })?;
    let ojval: Option<JsonVal> = cval2jval(&cval);
    ojval.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "unable to convert the cel value to json",
        )
    })
}

pub fn vref2ctx(ctx: &mut Context, name: &str, vref: ValueRef) -> Result<(), io::Error> {
    let oval: Option<CelVal> = match vref {
        ValueRef::Null => Some(CelVal::Null),
        ValueRef::Integer(i) => Some(CelVal::Int(i)),
        ValueRef::Real(f) => Some(CelVal::Float(f)),
        _ => None,
    };

    if let Some(val) = oval {
        return ctx.add_variable(name, val).map_err(io::Error::other);
    }

    match vref {
        ValueRef::Text(s) => {
            let utf8s: &str = std::str::from_utf8(s).unwrap_or_default();
            ctx.add_variable(name, utf8s).map_err(io::Error::other)
        }
        ValueRef::Blob(s) => ctx.add_variable(name, s).map_err(io::Error::other),
        _ => Err(io::Error::other("unexpected")),
    }
}

pub fn wtr2jsonval_sink<W>(mut wtr: W) -> impl FnMut(&JsonVal) -> Result<(), io::Error>
where
    W: Write,
{
    move |jval: &JsonVal| {
        serde_json::to_writer(&mut wtr, jval)?;
        writeln!(&mut wtr)
    }
}

pub fn stmt2jvalues2sink<S>(
    p: &Program,
    ctx: &mut Context,
    stmt: &mut Statement,
    sink: &mut S,
) -> Result<(), io::Error>
where
    S: FnMut(&JsonVal) -> Result<(), io::Error>,
{
    let icolnames = stmt.column_names();
    let colnames: Vec<String> = icolnames.map(|s| s.into()).collect();
    let mut rows: Rows = stmt.query(()).map_err(io::Error::other)?;
    loop {
        let orow: Option<&Row> = rows.next().map_err(io::Error::other)?;
        let Some(row) = orow else { return Ok(()) };

        for (idx, item) in colnames.iter().enumerate() {
            let vref: ValueRef = row.get_ref(idx).map_err(io::Error::other)?;
            let colname: &str = item;
            vref2ctx(ctx, colname, vref)?;
        }
        let jval: JsonVal = ctx2jval(p, ctx)?;
        sink(&jval)?;
    }
}

#[derive(Default)]
pub struct CelConfig {
    pub prog_source: String,
    pub environment: Option<Env>,
}

impl From<String> for CelConfig {
    fn from(prog_source: String) -> Self {
        Self {
            prog_source,
            environment: None,
        }
    }
}

impl CelConfig {
    pub fn to_program(&self) -> Result<Program, io::Error> {
        match &self.environment {
            Some(e) => e.compile(&self.prog_source),
            None => Program::compile(&self.prog_source),
        }
        .map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unable to compile: {e}"),
            )
        })
    }

    pub fn into_context<'a, 'b>(self) -> Context<'a, 'b> {
        match self.environment {
            Some(e) => Context::with_env(Arc::new(e)),
            None => Context::default(),
        }
    }
}

#[derive(Default)]
pub struct SqlConfig {
    pub path2sqlite: String,
    pub query: String,
}

#[derive(Default)]
pub struct Config {
    pub cel: CelConfig,
    pub sql: SqlConfig,
}

impl Config {
    pub fn sql2rows2cel2jsons2sink<S>(self, mut sink: S) -> Result<(), io::Error>
    where
        S: FnMut(&JsonVal) -> Result<(), io::Error>,
    {
        let prog: Program = self.cel.to_program()?;
        let mut ctx: Context = self.cel.into_context();

        let con: Connection = Connection::open(self.sql.path2sqlite).map_err(io::Error::other)?;
        let mut stmt: Statement = con.prepare(&self.sql.query).map_err(io::Error::other)?;
        stmt2jvalues2sink(&prog, &mut ctx, &mut stmt, &mut sink)?;
        stmt.finalize().map_err(io::Error::other)?;
        con.close().map_err(|p| p.1).map_err(io::Error::other)
    }
}

impl Config {
    pub fn sql2rows2cel2jsons2stdout(self) -> Result<(), io::Error> {
        let o = io::stdout();
        let mut ol = o.lock();
        let mut bwtr = BufWriter::new(&mut ol);
        let sink = wtr2jsonval_sink(&mut bwtr);
        self.sql2rows2cel2jsons2sink(sink)?;
        bwtr.flush()?;
        drop(bwtr);
        ol.flush()
    }
}
