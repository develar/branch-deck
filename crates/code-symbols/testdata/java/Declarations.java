package com.example.shapes;

/** A shape. */
public abstract class Declarations<T extends Comparable<T>> implements Shape {
  public static final int MAX = 10;
  private static int counter = 0;
  protected String name, label;
  double area;

  public Declarations(String name) {
    this.name = name;
  }

  @Override
  public abstract double area();

  private static <R> R convert(T value) {
    return null;
  }

  int packageMethod() {
    return counter;
  }

  public interface Visitor {
    int DEPTH = 3;
    void visit(Shape shape);
    default void leave() {}
    static Visitor empty() { return null; }
  }

  public enum Kind {
    ROUND, SQUARE {
      @Override
      public String toString() { return "square"; }
    };

    Kind() {}
    public String label() { return name(); }
  }

  public record Point(int x, int y) {
    public Point {
      if (x < 0) throw new IllegalArgumentException();
    }
    static Point origin() { return new Point(0, 0); }
  }

  public @interface Marker {
    String value() default "";
  }

  static class Nested {}
  class Inner {}
}

class PackagePrivate {}
