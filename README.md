# **D**e**C**om**P**iler

A work in progress decompiler, aiming to sacrifice some fine grained accuracy in order to prioritise a more usable, readable output.

## Building
This is a standard rust/cargo project, hence with rust installed simply run `cargo run -- -h` to get started.

## TODO
- Function detection without symbols
- Function calls when we don't have the implementation on hand
- ELF Files
- x86

## Example
```c
int other(int x) {
    return x + 3;
}

int main() {
    int a = 5;
    int b = a + 3;

    for (int i = 0; i < 5; i++) {
        if (i > 1) {
            if (a > b) {
                a = a + 3;
            }

            if (i == 2) {
                a -= b;
            } else {
                b = b - other(3);
                break;
            }
        }
    }

    return a;
}
```

Compiled with `gcc example.c -o example`, then decompiled with `cargo run -- example`:

```
func {
    frame 0 {
        var a: 4 bytes @ base + 12
        var b: 0 bytes @ base + 16
    }
    sp = sp - 16
    sp = &b
    return x0 + 3
}

func {
    frame 0 {
        var g: 4 bytes @ base + 12
        var f: 4 bytes @ base + 16
        var e: 4 bytes @ base + 20
        var d: 4 bytes @ base + 24
        var c: 4 bytes @ base + 28
        var a: 8 bytes @ base + 32
        var b: 8 bytes @ base + 40
        var h: 0 bytes @ base + 48
    }
    sp = sp - 48
    a = fp
    b = lr
    c = 0
    d = 5
    e = d + 3
    f = 0
    for f < 5; f = f + 1 {
        if f > 1 {
            if d > e {
                d = d + 3
            }
            if f != 2 {
                e = e - fn0(3)
                break
            }
            d = d - e
        }
    }
    fp = a
    lr = b
    sp = &h
    return d
}
```
