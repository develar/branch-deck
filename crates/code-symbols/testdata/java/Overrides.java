package com.example.windows;

import java.util.EventListener;
import java.util.List;

public class Overrides extends ToolWindowManagerEx implements Disposable, java.io.Serializable {
  @Override
  public void dispose() {}

  @java.lang.Override
  public String toString() {
    return "overrides";
  }

  public void open(String paneId, int... sizes) {}

  public final void closed() {}

  private void hidden() {}

  public static Overrides create(List<String> ids) {
    return null;
  }

  <T> T convert(T value, String[] names, java.util.Map<String, Integer> counts) {
    return value;
  }

  void listen() {
    Runnable task = new Runnable() {
      @Override
      public void run() {}
    };
    class Local extends Base<String> {
      @Override
      void accept(String value) {}
    }
  }

  public interface Listener extends EventListener {
    void changed(List<String> paths);

    default void reset() {}

    static Listener empty() {
      return null;
    }

    private void log() {}
  }

  enum Level implements Weighted {
    LOW {
      @Override
      public int weight() {
        return 1;
      }
    },
    HIGH;

    public int weight() {
      return 0;
    }
  }

  record Point(int x, int y) implements Comparable<Point> {
    @Override
    public int compareTo(Point other) {
      return 0;
    }
  }

  static final class Closed {
    public void member() {}
  }

  void fill(int[][] grid, String rows[]) {}
}
