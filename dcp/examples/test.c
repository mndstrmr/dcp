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
