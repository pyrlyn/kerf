//! Labelled fixture for the recall test. `recall.labels` lists, by hand, every
//! item definition and every call, macro call and impl target a reader expects.

use std::collections::HashMap;
use std::mem;

pub const LIMIT: usize = 8;
static GREETING: &str = "hi";

pub struct Point {
    x: i32,
    y: i32,
}

pub enum Shape {
    Circle(f64),
    Square(f64),
}

type Grid = HashMap<(i32, i32), Shape>;

pub trait Area {
    fn area(&self) -> f64;
    fn describe(&self) -> String {
        format!("area {}", self.area())
    }
}

impl Area for Shape {
    fn area(&self) -> f64 {
        match self {
            Shape::Circle(r) => square(*r) * 3.14,
            Shape::Square(s) => square(*s),
        }
    }
}

impl Point {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn origin() -> Self {
        Self::new(0, 0)
    }

    fn manhattan(&self) -> i32 {
        self.x.abs() + self.y.abs()
    }
}

fn square(v: f64) -> f64 {
    v * v
}

fn parse_all<T: std::str::FromStr>(items: &[&str]) -> Vec<T> {
    items.iter().filter_map(|s| s.parse::<T>().ok()).collect()
}

macro_rules! twice {
    ($e:expr) => {
        $e + $e
    };
}

mod geometry {
    pub fn unit() -> super::Point {
        super::Point::new(1, 1)
    }
}

pub fn run() -> i32 {
    let mut grid: Grid = HashMap::new();
    grid.insert((0, 0), Shape::Circle(1.0));
    let p = Point::origin();
    let q = geometry::unit();
    let n: Vec<i32> = parse_all::<i32>(&["1", "2"]);
    let _old = mem::take(&mut grid);
    let total = twice!(p.manhattan() + q.manhattan());
    println!("{GREETING} {}", n.len());
    total + LIMIT as i32 + square(2.0) as i32
}

// What tree-sitter cannot see: an item a macro defines, and a function named
// as a value. A language server resolves both.
macro_rules! getter {
    ($name:ident) => {
        pub fn $name() -> usize {
            LIMIT
        }
    };
}

getter!(limit);

pub fn scaled() -> f64 {
    let scale: fn(f64) -> f64 = square;
    scale(limit() as f64)
}
