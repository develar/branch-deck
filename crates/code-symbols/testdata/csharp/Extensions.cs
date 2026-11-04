using Example.Models;

namespace Example.Text;

public static class StringExtensions
{
    public static string Shout(this string text) => text.ToUpper();

    public static int Score(this Item item, int bonus) => item.Value + bonus;

    extension(Store store)
    {
        public void Reset() => store.Clear();

        public int Size => store.Count;
    }
}

public class Usage
{
    public void Run(Item item, string text)
    {
        item.Score(1);
        text.Shout();
    }
}

public class ItemTests
{
    [Fact]
    public void Scores() { }

    [Theory, InlineData(1)]
    public void ScoresBy(int bonus) { }

    [Experimental("BD001")]
    public void Later() { }
}
