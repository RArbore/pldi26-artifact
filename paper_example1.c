int example1(int y) {
    int x = -6;
    int z = 42;
    while (y < 10) {
        y = y + 1;
        x = x + 8;
        int lhs = ((x + y) + z) * y;
        int rhs = 2 * y + (y * y + z * y);
        if (lhs != rhs) {
            z = 24;
        }
        x = x - 8;
    }
    return z + 7;
}
