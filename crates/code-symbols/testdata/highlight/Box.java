package com.example.app;

import java.util.List;
import static java.util.Objects.requireNonNull;

/**
 * A box of items. See {@link List#add(Object)} and {@code size}.
 * @param <T> the type of an item
 */
@SuppressWarnings({"unchecked", "rawtypes"})
public final class Box<T extends Comparable<T>> implements Iterable<T> {
  public static final int LIMIT = 10;
  private static int created;
  private final List<T> items = new java.util.ArrayList<>();
  private int count;

  /** @throws IllegalStateException when full */
  public boolean add(T item, int times) {
    count++;
    this.count += times;
    created = Math.max(created, LIMIT);
    String text = "x=" + count + ", \"quoted\"\n" + 'c' + '\t';
    outer:
    for (int i = 0; i < times; i++) {
      if (i > Box.LIMIT) break outer;
      items.add(requireNonNull(item));
    }
    Runnable run = () -> System.out.println(text.length() + items.size() + 0x1F + 1.5e3);
    return items.isEmpty() || count > 0 && true != false;
  }

  @Override
  public java.util.Iterator<T> iterator() { return items.iterator(); }

  enum Color { RED, GREEN; Color next() { return this == RED ? GREEN : RED; } }

  record Point(int x, int y) { int sum() { return x + y; } }
}
