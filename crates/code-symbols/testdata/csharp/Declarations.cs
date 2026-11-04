using System;
using System.Collections.Generic;
using static System.Math;
using Items = System.Collections.Generic.List<int>;
global using Json = System.Text.Json.JsonSerializer;

namespace Example.Shapes;

/// <summary>A shape.</summary>
[Serializable]
public abstract class Shape : IComparable<Shape>, IShape
{
    public const int Max = 10;
    private static int counter;
    public static readonly string Empty = "";
    public string Name { get; private set; } = "";
    internal double area, perimeter;
    public event EventHandler? Changed;

    protected Shape(string name) { Name = name; }

    public abstract double Area();

    public virtual int CompareTo(Shape? other) => Area().CompareTo(other?.Area());

    private static R Convert<R>(object value) where R : class => (R)value;

    internal int AssemblyMethod() => counter;

    public interface IVisitor
    {
        const int Depth = 3;
        void Visit(Shape shape);
        static IVisitor Empty() => null!;
    }

    public enum Kind { Round, Square = 2 }

    public sealed record Point(int X, int Y)
    {
        public static Point Origin() => new(0, 0);
    }

    private class Nested { }

    public delegate void Handler(Shape sender, int code);

    [Obsolete("use Area")]
    public double Size => Area();

    public int this[int index] => index;
}

file class FileLocal { }

struct Pair { public int A; }

public static class Program
{
    public static void Main(string[] args) { }
}
