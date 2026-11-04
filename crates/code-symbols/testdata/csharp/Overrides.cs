namespace Example.Shapes;

public class Circle : Shape, IShape
{
    public Circle(double radius) : base("circle") { Radius = radius; }

    public Circle() : this(1) { }

    public double Radius { get; }

    public override double Area() => Radius * Radius * 3.14;

    public sealed override string ToString() => "circle";

    double IShape.Perimeter() => Radius * 2 * 3.14;

    static Circle() { }
}
