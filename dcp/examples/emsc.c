__attribute__((export_name("other"))) int other(int x) {
    return x + 4;
}

__attribute__((export_name("thing"))) int thing(int a, int b) {
    // for (int i = 0; i < 3; i++) {
    //     for (int k = 0; k < 4; k++)
    //         a += other(4);
    // }

    // // return a + b;
    // if (a < b) return a;
    // return b;

    int x = 3;
    if (a < b) {
        if (b < x) x++;

        if (a < x) {
            if (a) {
                x = 3;
                goto end;
            }

            if (x) {
                x = 4;
                goto end;
            }

            while (a < 100) {
                a++;
                x = x * 2;
            }
        }
        end:
    } else {
        x = 34;
        goto end;
    }

    return x;
}
