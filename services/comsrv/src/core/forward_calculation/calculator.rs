/// 转发计算执行引擎 - Forward Calculation Execution Engine
/// 
use std::collections::HashMap;
use std::str::FromStr;
use crate::utils::error::{ComSrvError, Result};
use crate::core::config::forward_calculation_config::{
    CalculationValue, ForwardCalculationRule
};

/// 表达式计算器 - Expression Calculator
pub struct ForwardCalculationEngine {
    /// 当前可用的变量值
    variables: HashMap<String, CalculationValue>,
}

impl ForwardCalculationEngine {
    /// 创建新的计算引擎
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    /// 设置变量值
    pub fn set_variable(&mut self, name: String, value: CalculationValue) {
        self.variables.insert(name, value);
    }

    /// 批量设置变量值
    pub fn set_variables(&mut self, variables: HashMap<String, CalculationValue>) {
        self.variables.extend(variables);
    }

    /// 获取变量值
    pub fn get_variable(&self, name: &str) -> Option<&CalculationValue> {
        self.variables.get(name)
    }

    /// 清空所有变量
    pub fn clear_variables(&mut self) {
        self.variables.clear();
    }

    /// 执行转发计算规则
    pub fn execute_rule(&mut self, rule: &ForwardCalculationRule) -> Result<CalculationValue> {
        // 验证所有源变量是否都有值
        for var_name in rule.sources.keys() {
            if !self.variables.contains_key(var_name) {
                return Err(ComSrvError::ConfigError(
                    format!("Missing variable value for: {}", var_name)
                ));
            }
        }

        // 解析并执行表达式
        self.evaluate_expression(&rule.expression)
    }

    /// 解析并计算表达式
    pub fn evaluate_expression(&self, expression: &str) -> Result<CalculationValue> {
        let expr = expression.trim();
        
        // 处理空表达式
        if expr.is_empty() {
            return Err(ComSrvError::ConfigError("Empty expression".to_string()));
        }

        // 解析表达式
        let tokens = self.tokenize(expr)?;
        let parsed = self.parse_tokens(tokens)?;
        self.evaluate_parsed_expression(parsed)
    }

    /// 词法分析 - 将表达式分解为tokens
    fn tokenize(&self, expr: &str) -> Result<Vec<Token>> {
        let mut tokens = Vec::new();
        let mut chars = expr.chars().peekable();
        
        while let Some(ch) = chars.next() {
            match ch {
                ' ' | '\t' | '\n' => continue, // 跳过空白字符
                '(' => tokens.push(Token::LeftParen),
                ')' => tokens.push(Token::RightParen),
                '+' => tokens.push(Token::Plus),
                '-' => tokens.push(Token::Minus),
                '*' => tokens.push(Token::Multiply),
                '/' => tokens.push(Token::Divide),
                '>' => {
                    if chars.peek() == Some(&'=') {
                        chars.next();
                        tokens.push(Token::GreaterEqual);
                    } else {
                        tokens.push(Token::Greater);
                    }
                },
                '<' => {
                    if chars.peek() == Some(&'=') {
                        chars.next();
                        tokens.push(Token::LessEqual);
                    } else {
                        tokens.push(Token::Less);
                    }
                },
                '=' => {
                    if chars.peek() == Some(&'=') {
                        chars.next();
                        tokens.push(Token::Equal);
                    } else {
                        return Err(ComSrvError::ConfigError("Invalid operator '='".to_string()));
                    }
                },
                '!' => {
                    if chars.peek() == Some(&'=') {
                        chars.next();
                        tokens.push(Token::NotEqual);
                    } else {
                        tokens.push(Token::Not);
                    }
                },
                '&' => {
                    if chars.peek() == Some(&'&') {
                        chars.next();
                        tokens.push(Token::And);
                    } else {
                        return Err(ComSrvError::ConfigError("Invalid operator '&'".to_string()));
                    }
                },
                '|' => {
                    if chars.peek() == Some(&'|') {
                        chars.next();
                        tokens.push(Token::Or);
                    } else {
                        return Err(ComSrvError::ConfigError("Invalid operator '|'".to_string()));
                    }
                },
                '0'..='9' | '.' => {
                    // 解析数字
                    let mut number = String::new();
                    number.push(ch);
                    
                    while let Some(&next_ch) = chars.peek() {
                        if next_ch.is_ascii_digit() || next_ch == '.' {
                            number.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }
                    
                    let value = f64::from_str(&number)
                        .map_err(|_| ComSrvError::ConfigError(format!("Invalid number: {}", number)))?;
                    tokens.push(Token::Number(value));
                },
                'a'..='z' | 'A'..='Z' | '_' => {
                    // 解析标识符或关键字
                    let mut identifier = String::new();
                    identifier.push(ch);
                    
                    while let Some(&next_ch) = chars.peek() {
                        if next_ch.is_alphanumeric() || next_ch == '_' {
                            identifier.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }
                    
                    // 检查是否是关键字
                    match identifier.to_uppercase().as_str() {
                        "AND" => tokens.push(Token::And),
                        "OR" => tokens.push(Token::Or),
                        "NOT" => tokens.push(Token::Not),
                        "TRUE" => tokens.push(Token::Boolean(true)),
                        "FALSE" => tokens.push(Token::Boolean(false)),
                        _ => tokens.push(Token::Identifier(identifier)),
                    }
                },
                _ => return Err(ComSrvError::ConfigError(format!("Invalid character: {}", ch))),
            }
        }
        
        Ok(tokens)
    }

    /// 语法分析 - 将tokens解析为表达式树
    fn parse_tokens(&self, tokens: Vec<Token>) -> Result<Expression> {
        let mut parser = Parser::new(tokens);
        parser.parse_expression()
    }

    /// 计算解析后的表达式
    fn evaluate_parsed_expression(&self, expr: Expression) -> Result<CalculationValue> {
        match expr {
            Expression::Number(n) => Ok(CalculationValue::Numeric(n)),
            Expression::Boolean(b) => Ok(CalculationValue::Boolean(b)),
            Expression::Variable(name) => {
                self.variables.get(&name)
                    .cloned()
                    .ok_or_else(|| ComSrvError::ConfigError(format!("Undefined variable: {}", name)))
            },
            Expression::BinaryOp { left, operator, right } => {
                let left_val = self.evaluate_parsed_expression(*left)?;
                let right_val = self.evaluate_parsed_expression(*right)?;
                self.apply_binary_operator(operator, left_val, right_val)
            },
            Expression::UnaryOp { operator, operand } => {
                let operand_val = self.evaluate_parsed_expression(*operand)?;
                self.apply_unary_operator(operator, operand_val)
            },
        }
    }

    /// 应用二元运算符
    fn apply_binary_operator(
        &self,
        operator: BinaryOperator,
        left: CalculationValue,
        right: CalculationValue,
    ) -> Result<CalculationValue> {
        match operator {
            BinaryOperator::Add => match (left, right) {
                (CalculationValue::Numeric(a), CalculationValue::Numeric(b)) => 
                    Ok(CalculationValue::Numeric(a + b)),
                _ => Err(ComSrvError::ConfigError("Addition requires numeric operands".to_string())),
            },
            BinaryOperator::Subtract => match (left, right) {
                (CalculationValue::Numeric(a), CalculationValue::Numeric(b)) => 
                    Ok(CalculationValue::Numeric(a - b)),
                _ => Err(ComSrvError::ConfigError("Subtraction requires numeric operands".to_string())),
            },
            BinaryOperator::Multiply => match (left, right) {
                (CalculationValue::Numeric(a), CalculationValue::Numeric(b)) => 
                    Ok(CalculationValue::Numeric(a * b)),
                _ => Err(ComSrvError::ConfigError("Multiplication requires numeric operands".to_string())),
            },
            BinaryOperator::Divide => match (left, right) {
                (CalculationValue::Numeric(a), CalculationValue::Numeric(b)) => {
                    if b == 0.0 {
                        Err(ComSrvError::ConfigError("Division by zero".to_string()))
                    } else {
                        Ok(CalculationValue::Numeric(a / b))
                    }
                },
                _ => Err(ComSrvError::ConfigError("Division requires numeric operands".to_string())),
            },
            BinaryOperator::And => match (left, right) {
                (CalculationValue::Boolean(a), CalculationValue::Boolean(b)) => 
                    Ok(CalculationValue::Boolean(a && b)),
                _ => Err(ComSrvError::ConfigError("AND requires boolean operands".to_string())),
            },
            BinaryOperator::Or => match (left, right) {
                (CalculationValue::Boolean(a), CalculationValue::Boolean(b)) => 
                    Ok(CalculationValue::Boolean(a || b)),
                _ => Err(ComSrvError::ConfigError("OR requires boolean operands".to_string())),
            },
            BinaryOperator::Greater => match (left, right) {
                (CalculationValue::Numeric(a), CalculationValue::Numeric(b)) => 
                    Ok(CalculationValue::Boolean(a > b)),
                _ => Err(ComSrvError::ConfigError("Comparison requires numeric operands".to_string())),
            },
            BinaryOperator::GreaterEqual => match (left, right) {
                (CalculationValue::Numeric(a), CalculationValue::Numeric(b)) => 
                    Ok(CalculationValue::Boolean(a >= b)),
                _ => Err(ComSrvError::ConfigError("Comparison requires numeric operands".to_string())),
            },
            BinaryOperator::Less => match (left, right) {
                (CalculationValue::Numeric(a), CalculationValue::Numeric(b)) => 
                    Ok(CalculationValue::Boolean(a < b)),
                _ => Err(ComSrvError::ConfigError("Comparison requires numeric operands".to_string())),
            },
            BinaryOperator::LessEqual => match (left, right) {
                (CalculationValue::Numeric(a), CalculationValue::Numeric(b)) => 
                    Ok(CalculationValue::Boolean(a <= b)),
                _ => Err(ComSrvError::ConfigError("Comparison requires numeric operands".to_string())),
            },
            BinaryOperator::Equal => match (left, right) {
                (CalculationValue::Numeric(a), CalculationValue::Numeric(b)) => 
                    Ok(CalculationValue::Boolean((a - b).abs() < f64::EPSILON)),
                (CalculationValue::Boolean(a), CalculationValue::Boolean(b)) => 
                    Ok(CalculationValue::Boolean(a == b)),
                _ => Err(ComSrvError::ConfigError("Comparison requires matching operand types".to_string())),
            },
            BinaryOperator::NotEqual => match (left, right) {
                (CalculationValue::Numeric(a), CalculationValue::Numeric(b)) => 
                    Ok(CalculationValue::Boolean((a - b).abs() >= f64::EPSILON)),
                (CalculationValue::Boolean(a), CalculationValue::Boolean(b)) => 
                    Ok(CalculationValue::Boolean(a != b)),
                _ => Err(ComSrvError::ConfigError("Comparison requires matching operand types".to_string())),
            },
        }
    }

    /// 应用一元运算符
    fn apply_unary_operator(
        &self,
        operator: UnaryOperator,
        operand: CalculationValue,
    ) -> Result<CalculationValue> {
        match operator {
            UnaryOperator::Not => match operand {
                CalculationValue::Boolean(b) => Ok(CalculationValue::Boolean(!b)),
                _ => Err(ComSrvError::ConfigError("NOT requires boolean operand".to_string())),
            },
            UnaryOperator::Minus => match operand {
                CalculationValue::Numeric(n) => Ok(CalculationValue::Numeric(-n)),
                _ => Err(ComSrvError::ConfigError("Unary minus requires numeric operand".to_string())),
            },
        }
    }
}

impl Default for ForwardCalculationEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// 词法分析token
#[derive(Debug, Clone, PartialEq)]
enum Token {
    // 字面量
    Number(f64),
    Boolean(bool),
    Identifier(String),
    
    // 运算符
    Plus,
    Minus,
    Multiply,
    Divide,
    And,
    Or,
    Not,
    
    // 比较运算符
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    Equal,
    NotEqual,
    
    // 括号
    LeftParen,
    RightParen,
}

/// 表达式抽象语法树
#[derive(Debug, Clone)]
enum Expression {
    Number(f64),
    Boolean(bool),
    Variable(String),
    BinaryOp {
        left: Box<Expression>,
        operator: BinaryOperator,
        right: Box<Expression>,
    },
    UnaryOp {
        operator: UnaryOperator,
        operand: Box<Expression>,
    },
}

/// 二元运算符
#[derive(Debug, Clone, Copy)]
enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    And,
    Or,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    Equal,
    NotEqual,
}

/// 一元运算符
#[derive(Debug, Clone, Copy)]
enum UnaryOperator {
    Not,
    Minus,
}

/// 语法分析器
struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, current: 0 }
    }

    fn parse_expression(&mut self) -> Result<Expression> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expression> {
        let mut expr = self.parse_and()?;

        while self.match_token(&Token::Or) {
            let operator = BinaryOperator::Or;
            let right = self.parse_and()?;
            expr = Expression::BinaryOp {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<Expression> {
        let mut expr = self.parse_equality()?;

        while self.match_token(&Token::And) {
            let operator = BinaryOperator::And;
            let right = self.parse_equality()?;
            expr = Expression::BinaryOp {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<Expression> {
        let mut expr = self.parse_comparison()?;

        while let Some(token) = self.peek() {
            let operator = match token {
                Token::Equal => BinaryOperator::Equal,
                Token::NotEqual => BinaryOperator::NotEqual,
                _ => break,
            };
            self.advance();
            let right = self.parse_comparison()?;
            expr = Expression::BinaryOp {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expression> {
        let mut expr = self.parse_term()?;

        while let Some(token) = self.peek() {
            let operator = match token {
                Token::Greater => BinaryOperator::Greater,
                Token::GreaterEqual => BinaryOperator::GreaterEqual,
                Token::Less => BinaryOperator::Less,
                Token::LessEqual => BinaryOperator::LessEqual,
                _ => break,
            };
            self.advance();
            let right = self.parse_term()?;
            expr = Expression::BinaryOp {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn parse_term(&mut self) -> Result<Expression> {
        let mut expr = self.parse_factor()?;

        while let Some(token) = self.peek() {
            let operator = match token {
                Token::Plus => BinaryOperator::Add,
                Token::Minus => BinaryOperator::Subtract,
                _ => break,
            };
            self.advance();
            let right = self.parse_factor()?;
            expr = Expression::BinaryOp {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn parse_factor(&mut self) -> Result<Expression> {
        let mut expr = self.parse_unary()?;

        while let Some(token) = self.peek() {
            let operator = match token {
                Token::Multiply => BinaryOperator::Multiply,
                Token::Divide => BinaryOperator::Divide,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            expr = Expression::BinaryOp {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expression> {
        if let Some(token) = self.peek() {
            match token {
                Token::Not => {
                    self.advance();
                    let operand = self.parse_unary()?;
                    return Ok(Expression::UnaryOp {
                        operator: UnaryOperator::Not,
                        operand: Box::new(operand),
                    });
                },
                Token::Minus => {
                    self.advance();
                    let operand = self.parse_unary()?;
                    return Ok(Expression::UnaryOp {
                        operator: UnaryOperator::Minus,
                        operand: Box::new(operand),
                    });
                },
                _ => {},
            }
        }

        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expression> {
        if let Some(token) = self.advance() {
            match token {
                Token::Number(n) => Ok(Expression::Number(*n)),
                Token::Boolean(b) => Ok(Expression::Boolean(*b)),
                Token::Identifier(name) => Ok(Expression::Variable(name.clone())),
                Token::LeftParen => {
                    let expr = self.parse_expression()?;
                    if !self.match_token(&Token::RightParen) {
                        return Err(ComSrvError::ConfigError("Expected ')' after expression".to_string()));
                    }
                    Ok(expr)
                },
                _ => Err(ComSrvError::ConfigError(format!("Unexpected token: {:?}", token))),
            }
        } else {
            Err(ComSrvError::ConfigError("Unexpected end of expression".to_string()))
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.current)
    }

    fn advance(&mut self) -> Option<&Token> {
        if self.current < self.tokens.len() {
            let token = &self.tokens[self.current];
            self.current += 1;
            Some(token)
        } else {
            None
        }
    }

    fn match_token(&mut self, expected: &Token) -> bool {
        if let Some(token) = self.peek() {
            if std::mem::discriminant(token) == std::mem::discriminant(expected) {
                self.advance();
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_arithmetic() {
        let mut engine = ForwardCalculationEngine::new();
        
        // 测试基本算术运算
        let result = engine.evaluate_expression("2 + 3 * 4").unwrap();
        assert_eq!(result, CalculationValue::Numeric(14.0));
        
        let result = engine.evaluate_expression("(2 + 3) * 4").unwrap();
        assert_eq!(result, CalculationValue::Numeric(20.0));
        
        let result = engine.evaluate_expression("10 / 2 - 3").unwrap();
        assert_eq!(result, CalculationValue::Numeric(2.0));
    }

    #[test]
    fn test_boolean_logic() {
        let mut engine = ForwardCalculationEngine::new();
        
        // 测试布尔逻辑
        let result = engine.evaluate_expression("true AND false").unwrap();
        assert_eq!(result, CalculationValue::Boolean(false));
        
        let result = engine.evaluate_expression("true OR false").unwrap();
        assert_eq!(result, CalculationValue::Boolean(true));
        
        let result = engine.evaluate_expression("NOT true").unwrap();
        assert_eq!(result, CalculationValue::Boolean(false));
    }

    #[test]
    fn test_comparison() {
        let mut engine = ForwardCalculationEngine::new();
        
        // 测试比较运算
        let result = engine.evaluate_expression("5 > 3").unwrap();
        assert_eq!(result, CalculationValue::Boolean(true));
        
        let result = engine.evaluate_expression("2 <= 2").unwrap();
        assert_eq!(result, CalculationValue::Boolean(true));
        
        let result = engine.evaluate_expression("1 == 1").unwrap();
        assert_eq!(result, CalculationValue::Boolean(true));
    }

    #[test]
    fn test_variables() {
        let mut engine = ForwardCalculationEngine::new();
        engine.set_variable("x".to_string(), CalculationValue::Numeric(10.0));
        engine.set_variable("y".to_string(), CalculationValue::Numeric(5.0));
        engine.set_variable("flag".to_string(), CalculationValue::Boolean(true));
        
        // 测试变量使用
        let result = engine.evaluate_expression("x + y").unwrap();
        assert_eq!(result, CalculationValue::Numeric(15.0));
        
        let result = engine.evaluate_expression("x > y AND flag").unwrap();
        assert_eq!(result, CalculationValue::Boolean(true));
    }

    #[test]
    fn test_complex_expression() {
        let mut engine = ForwardCalculationEngine::new();
        engine.set_variable("temp".to_string(), CalculationValue::Numeric(75.0));
        engine.set_variable("pressure".to_string(), CalculationValue::Numeric(1.2));
        engine.set_variable("alarm".to_string(), CalculationValue::Boolean(false));
        
        // 测试复杂表达式
        let result = engine.evaluate_expression(
            "(temp > 70 AND pressure < 1.5) OR NOT alarm"
        ).unwrap();
        assert_eq!(result, CalculationValue::Boolean(true));
    }
}