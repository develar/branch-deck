using System.Linq;
using Models = Example.Models;

namespace Example.App
{
    public class Box<T> where T : Models.Item, new()
    {
        private readonly List<T> items = new();
        private Models.Store store;

        public int Count => items.Count;

        public void Add(T item, int times = 1)
        {
            var created = new Models.Store("main", 3);
            created.Open(true);
            store.Close();
            this.store.Close();
            items.Add(item);
            Helpers.Log($"added {item.Name}", times);
            global::System.Console.WriteLine(nameof(item));
            if (item is Models.Special special) special.Boost(-1);
            foreach (Models.Item other in items.Where(x => x.Valid)) other.Touch();
            int local = 0;
            local++;
            item?.Touch();
            Func<int, int> twice = x => x * 2;
            var list = new List<Models.Item> { item };
            _ = Format(times: 2, value: "x");
        }

        private static string Format(int times, string value) => value;
    }
}
