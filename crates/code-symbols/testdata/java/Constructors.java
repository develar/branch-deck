package com.example.windows;

public class Constructors {
  public Constructors(String name, int... sizes) {
    this(name);
  }

  private <T> Constructors(T value) {
    super();
  }

  Constructors() {}

  public static class Implicit {}

  static class PackageImplicit {}

  private static class Hidden {}

  record Point(int x, int y) {
    public Point {
      if (x < 0) throw new IllegalArgumentException("x");
    }

    Point(int x) {
      this(x, 0);
    }
  }

  record Full(String name) {
    public Full(String name) {
      this.name = name;
    }
  }

  public record Plain(int size) {}

  interface Listener {}

  enum Level {
    LOW(1), HIGH(2);

    Level(int code) {}
  }

  @interface Marker {}

  void local() {
    class InMethod {
      InMethod(int size) {}
    }
  }

  {
    class InInitializer {
      InInitializer(long size) {}
    }
  }

  Object create() {
    Runnable task = new Base("task", 1) {
      @Override
      public void run() {
        new Constructors("inner");
      }
    };
    return new java.util.ArrayList<String>(10);
  }

  static class Holder<T> {
    public Holder(T value) {}

    private Holder(java.util.List<T> values) {}
  }

  static class Stamp {
    public Stamp(java.util.Date date) {}

    private Stamp(java.sql.Date date) {}
  }

  record Pair<A>(A first) {
    Pair(java.util.List<A> all) {
      this(all.get(0));
    }
  }

  record Generic<A>(A first) {
    Generic {}
  }
}
