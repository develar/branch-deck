package probe

import java.math.BigDecimal
import java.time.LocalDate
import java.math.RoundingMode
import java.util.Locale
import java.util.Objects

data class Entry(val date: LocalDate, val amount: BigDecimal, val note: String)

class Ledger(private val entries: MutableList<Entry> = mutableListOf()) {
    fun add(entry: Entry) {
        require(entry.note.isNotBlank()) { "note must not be blank" }
        entries.add(entry)
    }

    fun balance(): BigDecimal {
        return sumOf(entries)
    }

    fun balanceOn(date: LocalDate): BigDecimal {
        val upTo = entries.filter { !it.date.isAfter(date) }
        return sumOf(upTo)
    }

    fun largest(): Entry? = entries.maxByOrNull { it.amount }

    fun entriesBetween(from: LocalDate, to: LocalDate): List<Entry> {
        return entries.filter { !it.date.isBefore(from) && !it.date.isAfter(to) }
    }

    fun formatted(): String {
        return entries.joinToString("\n") { formatEntry(it) }
    }

    private fun sumOf(list: List<Entry>): BigDecimal {
        var total = BigDecimal.ZERO
        for (e in list) {
            total = total.add(e.amount)
        }
        return total.setScale(2, RoundingMode.HALF_UP)
    }

    private fun formatEntry(entry: Entry): String {
        return String.format(Locale.ROOT, "%s %10s %s", entry.date, entry.amount.toPlainString(), entry.note)
    }

    fun averageAmount(): BigDecimal {
        if (entries.isEmpty()) return BigDecimal.ZERO
        return sumOf(entries).divide(BigDecimal(entries.size), 2, RoundingMode.HALF_UP)
    }
}
