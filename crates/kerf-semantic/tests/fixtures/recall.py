"""Labelled fixture for the Python recall test; `recall_py.labels` lists, by
hand, every definition and every call a reader expects."""

import math
from dataclasses import dataclass

LIMIT = 8
GREETING = "hi"


@dataclass
class Point:
    x: int
    y: int

    def manhattan(self) -> int:
        return abs(self.x) + abs(self.y)

    @classmethod
    def origin(cls) -> "Point":
        return cls(0, 0)


class Circle(Point):
    def __init__(self, r: float) -> None:
        super().__init__(0, 0)
        self.r = r

    def area(self) -> float:
        return math.pi * square(self.r)


def square(v: float) -> float:
    return v * v


async def fetch(n: int) -> list[int]:
    return [square(i) for i in range(n)]


def run() -> int:
    p = Point.origin()
    c = Circle(1.0)
    total = p.manhattan() + int(c.area())
    print(f"{GREETING} {len(str(total))}")
    return total + LIMIT
