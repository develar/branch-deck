using System;
using System.Collections.Generic;
using static System.Math;
using Items = System.Collections.Generic.List<int>;

namespace Example.App;

/// <summary>A box of items. See <see cref="List{T}.Add(T)"/> and <paramref name="item"/>.</summary>
/// <typeparam name="T">the type of an item</typeparam>
[Serializable, Obsolete("old", error: false)]
public sealed partial class Box<T> : IEnumerable<T> where T : IComparable<T>
{
    public const int Limit = 10;
    private static int created;
    private readonly List<T> items = new();
    private int count;
    public int Count { get => count; private set => count = value; }
    public event EventHandler? Changed;

    /// <exception cref="InvalidOperationException">when full</exception>
    public bool Add(T item, int times = 1)
    {
        count++;
        this.count += times;
        created = Max(created, Limit);
        var text = $"x={count,4:D}, {{braces}} \"quoted\"\n" + @"C:\path ""q""" + 'c' + '\t' + "\x41\u0042";
        var raw = """
            raw "text" \n
            """;
        outer:
        for (int i = 0; i < times; i++)
        {
            if (i > Box<T>.Limit) goto outer;
            items.Add(item ?? throw new ArgumentNullException(nameof(item)));
        }
        Func<int, int> twice = x => x * 2;
        Changed?.Invoke(this, EventArgs.Empty);
#if DEBUG
        Console.WriteLine(text.Length + items.Count + 0x1F + 1.5e3m + twice(1));
#endif
        return items.Count == 0 || count > 0 && true != false && raw is not null;
    }

    public static Box<T> Create() => new Box<T>();

    public IEnumerator<T> GetEnumerator() => items.GetEnumerator();

    System.Collections.IEnumerator System.Collections.IEnumerable.GetEnumerator() => GetEnumerator();

    #region Nested
    enum Color { Red, Green }

    record Point(int X, int Y) { public int Sum() => X + Y; }
    #endregion
}
