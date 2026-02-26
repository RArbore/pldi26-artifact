int example2(int x) {
    int y = x;
    while (y < 10) {
        int xt = x;
        x = y * y + y * 5;
        y = xt * (y + 5 + 0);
    }
    return x - y;
}
