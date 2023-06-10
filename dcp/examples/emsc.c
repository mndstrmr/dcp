int other(int x) {
    return x + 4;
}

int thing(int a, int b) {
    for (int i = 0; i < 3; i++) {
        for (int k = 0; k < 4; k++)
            a += other(4);
    }

    // return a + b;
    if (a < b) return a;
    return b;
}
