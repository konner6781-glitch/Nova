use crate::tokens::SourceLocation;

#[derive(Debug, Clone)]
pub struct Program {
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub enum Statement {
    Located {
        location: SourceLocation,
        statement: Box<Statement>,
    },

    Variable {
        name: String,
        value: Expression,
    },

    Assignment {
        name: String,
        operator: AssignmentOperator,
        value: Expression,
    },

    IndexAssignment {
        name: String,
        indices: Vec<Expression>,
        operator: AssignmentOperator,
        value: Expression,
    },

    UnpackAssignment {
        names: Vec<String>,
        value: Expression,
    },

    Expression(Expression),

    If {
        condition: Expression,
        body: Vec<Statement>,
        elif_branches: Vec<(Expression, Vec<Statement>)>,
        else_body: Option<Vec<Statement>>,
    },

    Loop {
        settings: LoopSettings,
        body: Vec<Statement>,
    },

    Function {
        name: String,
        parameters: Vec<String>,
        body: Vec<Statement>,
    },

    Return(Option<Vec<Expression>>),

    Break,

    Import {
        module: String,
        item: Option<String>,
    },

    Stop,
}

#[derive(Debug, Clone)]
pub struct LoopSettings {
    pub condition: Option<Expression>,
    pub max_iterations: Option<Expression>,
    pub delay: Option<Expression>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentOperator {
    Assign,
    Add,
    Subtract,
    Multiply,
    Divide,
}

#[derive(Debug, Clone)]
pub enum Expression {
    Located {
        location: SourceLocation,
        expression: Box<Expression>,
    },

    Integer(i64),

    Float(f64),

    String(String),

    Boolean(bool),

    Identifier(String),

    List(Vec<Expression>),

    Dictionary(Vec<(Expression, Expression)>),

    Index {
        object: Box<Expression>,
        index: Box<Expression>,
    },

    Member {
        object: Box<Expression>,
        name: String,
    },

    Unary {
        operator: UnaryOperator,
        operand: Box<Expression>,
    },

    Binary {
        left: Box<Expression>,
        operator: BinaryOperator,
        right: Box<Expression>,
    },

    Call {
        name: String,
        arguments: Vec<Expression>,
    },

    Grouped(Box<Expression>),
}

impl Statement {
    pub fn unlocated(&self) -> &Statement {
        match self {
            Statement::Located { statement, .. } => statement.unlocated(),
            statement => statement,
        }
    }

    pub fn location(&self) -> Option<SourceLocation> {
        match self {
            Statement::Located { location, .. } => Some(*location),
            _ => None,
        }
    }
}

impl Expression {
    pub fn unlocated(&self) -> &Expression {
        match self {
            Expression::Located { expression, .. } => expression.unlocated(),
            expression => expression,
        }
    }

    pub fn location(&self) -> Option<SourceLocation> {
        match self {
            Expression::Located { location, .. } => Some(*location),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum UnaryOperator {
    Negative,
    Not,
}

#[derive(Debug, Clone, Copy)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,

    Equal,
    NotEqual,

    LessThan,
    GreaterThan,
    LessEqual,
    GreaterEqual,

    And,
    Or,
}
