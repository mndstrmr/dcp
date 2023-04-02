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
                continue;
            }
        }
    }

    return a;
}
