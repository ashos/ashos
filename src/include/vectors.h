#ifndef __VECTORS_H_
#define __VECTORS_H_

#include<stdlib.h>
#include<string.h>

typedef struct v_int {
  int* data;
  size_t len;
} v_int;

typedef struct string {
  char* str;
  size_t len; // does not include NULL terminator - "hello" is length 5
} string;

typedef struct v_str {
  string* data;
  size_t len;
} v_str;


v_int* v_int_new(size_t size) {
  v_int* v = malloc(sizeof(v_int));
  v->len = size;
  v->data = malloc(sizeof(int) * size);
  return v;
}

void v_int_push(v_int* vec, int push) {
  vec->len++;
  vec->data = realloc(vec->data, sizeof(int) * vec->len);
  vec->data[vec->len - 1] = push;
}

void v_int_pop(v_int* vec) {
  vec->len--;
  vec->data = realloc(vec->data, sizeof(int) * vec->len);
}

void v_int_free(v_int* vec) {
  free(vec->data);
  vec->len = 0;
  free(vec);
}

void v_int_reverse(v_int* vec) {
  int* arr = vec->data;
  int n = vec->len;
  for (int low = 0, high = n - 1; low < high; low++, high--)
    {
      int temp = arr[low];
      arr[low] = arr[high];
      arr[high] = temp;
    }
}

void v_int_cat(v_int* vec, v_int* cat) {
  size_t index = vec->len;
  vec->len = vec->len + cat->len;
  vec->data = realloc(vec->data, vec->len * sizeof(int));
  for (size_t i = 0; i < cat->len; i++) {
    vec->data[i + index] = cat->data[i];
  }
}

v_int_erase(v_int* vec, size_t pos, int n) {
  for (size_t i = pos; i < vec->len - n; i++) {
    vec[pos] = vec[pos+i];
  }
  vec->len -= n;
  vec->data = realloc(vec->data, sizeof(int) * vec->len);
}

v_str* v_str_new(size_t size) {
  v_str* v = malloc(sizeof(v_str));
  v->len = size;
  v->data = malloc(sizeof(string) * size);
  return v;
}

void v_str_push(v_str* vec, string push) {
  vec->len++;
  vec->data = realloc(vec->data, sizeof(string) * vec->len);
  vec->data[vec->len - 1] = push;
}

void v_str_pop(v_str* vec) {
  vec->len--;
  vec->data = realloc(vec->data, sizeof(string) * vec->len);
}

void v_str_free(v_str* vec) {
  free(vec->data);
  vec->len = 0;
  free(vec);
}


void str_set(string* str, const char* txt) {
  size_t txtlen = strlen(txt);
  str->str = malloc(sizeof(char) * (txtlen + 1));
  str->len = txtlen;
  strcpy(str->str, txt);
}

void str_copy(string* str, string* from) {
  str->len = from->len;
  str->str = malloc(sizeof(char) * (from->len + 1));
  strcpy(str->str, from->str);
}

void str_free(string* str) {
  free(str->str);
  str->len = 0;
}

#endif
