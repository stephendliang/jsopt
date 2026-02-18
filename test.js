// Ground truth test file
const x = 42;
let y = "hello";
var z = true;

function add(a, b) {
    return a + b;
}

const arrow = (n) => n * 2;

class Point {
    constructor(x, y) {
        this.x = x;
        this.y = y;
    }
    static origin() {
        return new Point(0, 0);
    }
    get magnitude() {
        return Math.sqrt(this.x ** 2 + this.y ** 2);
    }
}

if (x > 10) {
    console.log("big");
} else {
    console.log("small");
}

for (let i = 0; i < 10; i++) {
    if (i % 2 === 0) continue;
    console.log(i);
}

const obj = { a: 1, b: 2, ...{ c: 3 } };
const [first, ...rest] = [1, 2, 3];
const { a: renamed, b } = obj;

try {
    throw new Error("oops");
} catch (e) {
    console.error(e);
} finally {
    console.log("done");
}

switch (x) {
    case 1: break;
    case 2: y = "two"; break;
    default: y = "other";
}

const ternary = x > 5 ? "yes" : "no";
const nullish = null ?? "fallback";
const chain = obj?.a?.toString();

async function fetchData(url) {
    const resp = await fetch(url);
    return resp.json();
}

function* gen() {
    yield 1;
    yield 2;
}

export { add, Point };
export default arrow;
