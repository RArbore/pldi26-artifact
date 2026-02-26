use core::fmt::{Display, Formatter, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuncAST {
    pub name: String,
    pub params: Vec<String>,
    pub body: StmtAST,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StmtAST {
    Block(Vec<StmtAST>),
    Assign(String, ExprAST),
    IfElse(ExprAST, Box<StmtAST>, Box<StmtAST>),
    While(ExprAST, Box<StmtAST>),
    Return(ExprAST),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprAST {
    Number(i64),
    Variable(String),
    Unary(UnaryOp, Box<ExprAST>),
    Binary(BinaryOp, Box<ExprAST>, Box<ExprAST>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    EE,
    NE,
    LT,
    LE,
    GT,
    GE,
}

impl Display for FuncAST {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "fn {}(", self.name)?;
        for idx in 0..self.params.len() {
            if idx != 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", self.params[idx])?;
        }
        write!(f, ") {}", self.body)
    }
}

impl Display for StmtAST {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            StmtAST::Block(stmts) => {
                write!(f, "{{ ")?;
                for stmt in stmts {
                    stmt.fmt(f)?;
                    write!(f, " ")?;
                }
                write!(f, "}}")
            }
            StmtAST::Assign(var, expr) => write!(f, "{} = {};", var, expr),
            StmtAST::IfElse(cond, then_stmt, else_stmt) => {
                write!(
                    f,
                    "if {} {{ {} }} else {{ {} }}",
                    cond, then_stmt, else_stmt
                )
            }
            StmtAST::While(cond, body) => write!(f, "while {} {{ {} }}", cond, body),
            StmtAST::Return(expr) => write!(f, "return {};", expr),
        }
    }
}

impl Display for ExprAST {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            ExprAST::Number(num) => num.fmt(f),
            ExprAST::Variable(name) => name.fmt(f),
            ExprAST::Unary(op, input) => write!(f, "{}{}", op, input),
            ExprAST::Binary(op, lhs, rhs) => write!(f, "({} {} {})", lhs, op, rhs),
        }
    }
}

impl Display for UnaryOp {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            UnaryOp::Neg => "-".fmt(f),
            UnaryOp::Not => "!".fmt(f),
        }
    }
}

impl Display for BinaryOp {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            BinaryOp::Add => "+".fmt(f),
            BinaryOp::Sub => "-".fmt(f),
            BinaryOp::Mul => "*".fmt(f),
            BinaryOp::EE => "==".fmt(f),
            BinaryOp::NE => "!=".fmt(f),
            BinaryOp::LT => "<".fmt(f),
            BinaryOp::LE => "<=".fmt(f),
            BinaryOp::GT => ">".fmt(f),
            BinaryOp::GE => ">=".fmt(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::grammar::ProgramParser;

    use super::*;

    #[test]
    fn parse1() {
        let program = r#"
fn test1(x) return x;
"#;
        let parsed = ProgramParser::new().parse(&program).unwrap();
        assert_eq!(
            parsed,
            vec![FuncAST {
                name: "test1".into(),
                params: vec!["x".into()],
                body: StmtAST::Return(ExprAST::Variable("x".into()))
            }]
        );
    }

    #[test]
    fn parse2() {
        use BinaryOp::*;
        use ExprAST::*;
        use StmtAST::*;
        let program = r#"
fn test2(x, y) { while x < 7 { x = x + 1; } if y < x { return y; } return x + 9; }
"#;
        let parsed = ProgramParser::new().parse(&program).unwrap();
        assert_eq!(
            parsed,
            vec![FuncAST {
                name: "test2".into(),
                params: vec!["x".into(), "y".into()],
                body: Block(vec![
                    While(
                        Binary(LT, Box::new(Variable("x".into())), Box::new(Number(7))),
                        Box::new(Block(vec![Assign(
                            "x".into(),
                            Binary(Add, Box::new(Variable("x".into())), Box::new(Number(1)))
                        )]))
                    ),
                    IfElse(
                        Binary(
                            LT,
                            Box::new(Variable("y".into())),
                            Box::new(Variable("x".into()))
                        ),
                        Box::new(Block(vec![Return(Variable("y".into()))])),
                        Box::new(Block(vec![]))
                    ),
                    Return(Binary(
                        Add,
                        Box::new(Variable("x".into())),
                        Box::new(Number(9))
                    ))
                ])
            }]
        );
    }
}
