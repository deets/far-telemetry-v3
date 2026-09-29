#pragma once
#include <string>

// Number base
#define DEC 10

#define F(a) a

class String : public std::string {
public:
  using std::string::string;
  String(int, int base = DEC);

  friend String operator+(const char*, const String&);
};

String operator+(const char *, const String &);
