package com.example.delegation;

import com.example.base.Base;

public class ConstructorDelegation extends Base implements Runnable {
  public ConstructorDelegation(int size) {
    super(size, "delegation");
  }

  ConstructorDelegation() {
    this(0);
  }

  public void run() {}

  static class Qualified extends com.example.base.Base<String> {
    Qualified() {
      super(1);
    }
  }

  static class OnlyInterfaces implements Runnable {
    OnlyInterfaces() {
      super();
    }

    public void run() {}
  }

  class Inner extends Outer.Nested {
    Inner(Outer outer) {
      outer.super();
    }
  }

  enum Level {
    LOW;

    Level() {
      this(0);
    }

    Level(int code) {}
  }

  record Point(int x, int y) {
    Point(int x) {
      this(x, 0);
    }
  }
}
