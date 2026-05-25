// Test file for variadic wildcard __
void func(void *para) {
    my_func(para, x1, x2, x3);   // [1] para at pos 0
    my_func(x1, para, x2, x3);   // [2] para at pos 1
    my_func(x1, x2, para, x3);   // [3] para at pos 2
    my_func(x1, x2, x3, para);   // [4] para at pos 3
}

void multi_func(void *p) {
    // Different functions, para at different positions
    func_a(p, a, b);              // [5] p at pos 0 of func_a
    func_b(a, p, b);              // [6] p at pos 1 of func_b
    func_c(a, b, p);              // [7] p at pos 2 of func_c
}

void order_test(void *a, void *b) {
    // Test that ordering is preserved with __
    ordered_func(a, x, b);        // [8] a before b
    ordered_func(b, x, a);        // [9] b before a
}

void exact_match_test(void) {
    // These should NOT match with exact argument count
    func2args(x, y);              // [10] 2 args
    func3args(x, y, z);           // [11] 3 args
    func4args(x, y, z, w);        // [12] 4 args
}

void subexpr_test(void *p) {
    // __ should NOT match sub-expressions (unlike _(...))
    func(p, x, y);                // [13] direct arg
    func(x->p, y);                // [14] NOT direct arg (field access)
    func(p + 1, y);               // [15] NOT direct arg (binary expr)
}
