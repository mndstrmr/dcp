# **D**e**C**om**P**iler

A work in progress decompiler, aiming to sacrifice some fine grained accuracy in order to prioritise a more usable, readable output.

## Building
This is a standard rust/cargo project, hence with rust installed simply run `cargo run -- -h` to get started.

## TODO
- Function detection without symbols
- Function calls
- ELF Files
- x86
- Improve operand size accuracy
- Named memory locations

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
block {
    sp = (sp - 16)
    *d (sp + 12) = x0
    x0 = (*d (sp + 12) + 3)
    sp = (sp + 16)
    return x0
}

block {
    sp = (sp - 48)
    *q (sp + 32) = fp
    *q (sp + 40) = lr
    *d (sp + 28) = 0
    *d (sp + 24) = 5
    *d (sp + 20) = (*d (sp + 24) + 3)
    *d (sp + 16) = 0
    for (*d (sp + 16) < 5); *d (sp + 16) = (*d (sp + 16) + 1) {
        if (*d (sp + 16) > 1) {
            if (*d (sp + 24) > *d (sp + 20)) {
                *d (sp + 24) = (*d (sp + 24) + 3)
            }
            if (*d (sp + 16) != 2) {
                *d (sp + 12) = *d (sp + 20)
                *d (sp + 20) = (*d (sp + 12) - fn0(3))
                break
            }
            *d (sp + 24) = (*d (sp + 24) - *d (sp + 20))
        }
    }
    x0 = *d (sp + 24)
    fp = *q (sp + 32)
    lr = *q (sp + 40)
    sp = (sp + 48)
    return x0
}
```
