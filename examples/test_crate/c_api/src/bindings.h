typedef struct my_struct {
  int x;
  int y;
} my_struct;
my_struct *my_struct_new();
void my_struct_free(my_struct *obj);
int add(int, int, void *);
char *greet(char *, void *);