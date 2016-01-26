extern crate itertools;

use itertools::Itertools;

use std::collections::HashMap;
use std::fmt;

pub mod kind;
pub mod kind_var;
mod parse;
mod prim;

#[derive(Debug, Clone)]
pub enum EvalError {
    StackUnderflow,
    NotFound(String),
    TypeMismatch,
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            EvalError::StackUnderflow => write!(f, "stack underflow"),
            EvalError::NotFound(ref name) => write!(f, "could not find `{}`", name),
            EvalError::TypeMismatch => write!(f, "type mismatch"),
        }
    }
}

pub type EvalResult = Result<Stack, EvalError>;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Number(i32),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Value::Bool(x) => write!(f, "{}", x),
            Value::Number(x) => write!(f, "{}", x),
        }
    }
}

pub type PrimFn = fn(Stack, &Words) -> EvalResult;

// The type `fn(T, &U) -> V` does not implement `Debug`, `Clone`, or
// `PartialEq`, so to enable `#[derive(..)]` to work for `Term`, we implement
// these for a wrapper struct.
#[derive(Copy)]
pub struct Prim {
    f: PrimFn,
}

impl fmt::Debug for Prim {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Prim(..)")
    }
}

impl Clone for Prim {
    fn clone(&self) -> Prim { Prim { f: self.f } }
}

impl PartialEq for Prim {
    fn eq(&self, other: &Prim) -> bool {
        &self.f as *const _ == &other.f as *const _
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Term {
    Push(Value),
    Quote(Stack),
    Call(String),
    Prim(Prim),
}

impl Term {
    pub fn call<S: ToString>(name: S) -> Term {
        Term::Call(name.to_string())
    }

    pub fn prim(f: PrimFn) -> Term {
        Term::Prim(Prim { f: f })
    }
}

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Term::Push(value) => write!(f, "{}", value),
            Term::Quote(ref stack) if stack.terms.is_empty() => write!(f, "[ ]"),
            Term::Quote(ref stack) => write!(f, "[ {} ]", stack),
            Term::Call(ref name) => write!(f, "{}", name),
            Term::Prim(_) => write!(f, "<prim>"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Words {
    defs: HashMap<String, Term>,
}

impl Words {
    pub fn empty() -> Words {
        Words {
            defs: HashMap::new(),
        }
    }

    pub fn standard() -> Words {
        let mut words = Words::empty();

        words.define("words", Term::prim(prim::words)); // (A ~> A)

        words.define("dup", Term::prim(prim::dup)); // (A b -> A b b)
        words.define("pop", Term::prim(prim::pop)); // (A b -> A)
        words.define("swap", Term::prim(prim::swap)); // (A b c -> A c b)
        words.define("apply", Term::prim(prim::apply)); // (A (A -> B) -> B)
        words.define("quote", Term::prim(prim::quote)); // (A b -> A (C -> C b))
        words.define("compose", Term::prim(prim::compose)); // (A (B -> C) (C -> D) -> A (B -> D)))

        words.define("if", Term::prim(prim::if_)); // (A bool (A -> B) (A -> B) -> B)

        words.define("eq", Term::prim(prim::eq)); // (A num num -> A bool)
        words.define("and", Term::prim(prim::and)); // (A bool bool -> A bool)
        words.define("or", Term::prim(prim::or)); // (A bool bool -> A bool)
        words.define("not", Term::prim(prim::not)); // (A bool -> A bool)

        words.define("+", Term::prim(prim::add)); // (A num num -> A num)
        words.define("-", Term::prim(prim::sub)); // (A num num -> A num)
        words.define("*", Term::prim(prim::mul)); // (A num num -> A num)
        words.define("/", Term::prim(prim::div)); // (A num num -> A num)
        words.define("%", Term::prim(prim::rem)); // (A num num -> A num)

        words
    }

    pub fn define<S: Into<String>>(&mut self, name: S, x: Term) {
        self.defs.insert(name.into(), x);
    }

    pub fn lookup(&self, name: &str) -> Option<&Term> {
        self.defs.get(name)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stack {
    terms: Vec<Term>,
}

impl Stack {
    pub fn new(terms: Vec<Term>) -> Stack {
        Stack { terms: terms }
    }

    pub fn empty() -> Stack {
        Stack::new(vec![])
    }

    fn push(mut self, term: Term) -> Stack {
        self.terms.push(term);
        self
    }

    fn pop(mut self) -> Result<(Stack, Term), EvalError> {
        match self.terms.pop() {
            Some(term) => Ok((self, term)),
            None => Err(EvalError::StackUnderflow),
        }
    }

    fn pop_bool(self) -> Result<(Stack, bool), EvalError> {
        let (stack, term) = try!(self.pop());
        match term {
            Term::Push(Value::Bool(x)) => Ok((stack, x)),
            _ => Err(EvalError::TypeMismatch),
        }
    }

    fn pop_number(self) -> Result<(Stack, i32), EvalError> {
        let (stack, term) = try!(self.pop());
        match term {
            Term::Push(Value::Number(x)) => Ok((stack, x)),
            _ => Err(EvalError::TypeMismatch),
        }
    }

    fn pop_quote(self) -> Result<(Stack, Stack), EvalError> {
        let (stack, term) = try!(self.pop());
        match term {
            Term::Quote(quoted) => Ok((stack, quoted)),
            _ => Err(EvalError::TypeMismatch),
        }
    }

    fn peek(&self) -> Result<&Term, EvalError> {
        match self.terms.last() {
            Some(term) => Ok(term),
            None => Err(EvalError::StackUnderflow),
        }
    }

    fn eval_term(self, words: &Words, term: Term) -> EvalResult {
        match term {
            Term::Push(value) => Ok(self.push(Term::Push(value))),
            Term::Quote(stack) => Ok(self.push(Term::Quote(stack))),
            Term::Call(name) => {
                match words.lookup(&name) {
                    Some(term) => self.eval_term(words, term.clone()),
                    None => Err(EvalError::NotFound(name)),
                }
            },
            Term::Prim(Prim { f }) => f(self, words),
        }
    }

    fn eval_stack(mut self, words: &Words, quote: Stack) -> EvalResult {
        let mut terms = quote.terms.into_iter();
        while let Some(term) = terms.next() {
            self = try!(self.eval_term(words, term));
        }
        Ok(self)
    }
}

pub fn eval(stack: Stack, words: &Words) -> EvalResult {
    Stack::empty().eval_stack(words, stack)
}

impl fmt::Display for Stack {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.terms.iter().format(" ", |t, f| f(t)))
    }
}

#[cfg(test)]
mod tests {
    mod prim {
        mod eq {
            use Prim;
            use prim;

            #[test]
            fn test_same() {
                assert_eq!(Prim { f: prim::eq }, Prim { f: prim::eq });
                assert_eq!(Prim { f: prim::add }, Prim { f: prim::add });
            }

            #[test]
            fn test_different() {
                assert!(Prim { f: prim::eq } != Prim { f: prim::add });
                assert!(Prim { f: prim::add } != Prim { f: prim::eq });
            }
        }
    }
}
