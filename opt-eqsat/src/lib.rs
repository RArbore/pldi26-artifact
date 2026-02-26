use lalrpop_util::lalrpop_mod;

pub mod analyses;
pub mod ast;
pub mod domains;
pub mod generate;
pub mod rewrites;
pub mod ssa;

lalrpop_mod!(pub grammar);
