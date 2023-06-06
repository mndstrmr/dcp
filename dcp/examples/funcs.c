int a(int x, int y, int z) {
    return x + y + z;
}

int b(int x, int y) {
    if (x < 3) return a(x, 7, y);
    return b(x - 1, 2) + 1;
}

// int c(int x) {
//     return b(x, x) + a(x, 4, 5);
// }

int main() {
    // return c(3);
}
