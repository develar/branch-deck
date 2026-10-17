package com.example.refs;

import java.util.List;
import java.util.function.*;
import static java.util.Objects.requireNonNull;
import static org.junit.Assert.*;

/**
 * Javadoc names Phantom and {@link Ghost}.
 */
@SuppressWarnings(value = "unused")
public class References {
  // Phantom in a line comment
  /* Ghost in a block comment */
  private final Canvas canvas = new Canvas();

  void render(List<Shape> shapes, int scale) {
    String title = "Phantom " + shapes.size();
    int count = 0;
    for (int i = 0; i < scale; i++) {
      count += i;
    }
    for (Shape shape : shapes) {
      canvas.draw(shape, count);
    }
    shapes.forEach(s -> canvas.outline(s));
    shapes.stream().map(Shape::area).filter((Double area) -> area > THRESHOLD);
    try (var writer = canvas.open(); Output output = writer.output()) {
      output.write(title);
    } catch (IOException | RuntimeException e) {
      log(e.getMessage());
    }
    if (shapes.get(0) instanceof Circle circle && circle.radius() > 0) {
      canvas.circle(circle);
    }
    Object result = switch (scale) {
      case 0 -> Kind.NONE;
      default -> {
        yield requireNonNull(title);
      }
    };
    Runnable job = new Runnable() {
      @Override
      public void run() { flush(); }
    };
    java.util.Collections.emptyList();
    outer:
    for (Shape a : shapes) {
      break outer;
    }
  }

  private static final double THRESHOLD = 0.5;
}
