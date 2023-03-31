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
int main() {
    int a;
    int b = 2;
    if (b == 1) {
        if (b == 3) {
            a = 1;
        } else {
            a = 3;
            for (int i = 0; i < 3; i++) {
                a += i;
            }
        }
    } else {
        a = 2;
    }
    return a;
}
```

Compiled with `gcc example.c -o eaxmple`, then decompiled with `cargo run -- example`:

```
block {
    sp = (sp - 16)
    *(sp + 12) = 0
    *(sp + 4) = 2
    if (*(sp + 4) != 1) {
        *(sp + 8) = 2
    } else {
        if (*(sp + 4) != 3) {
            *(sp + 8) = 3
            *sp = 0
            for (*sp < 3); *sp = (*sp + 1) {
                *(sp + 8) = (*(sp + 8) + *sp)
            }
        } else {
            *(sp + 8) = 1
        }
    }
    x0 = *(sp + 8)
    sp = (sp + 16)
    return x0
}
```
