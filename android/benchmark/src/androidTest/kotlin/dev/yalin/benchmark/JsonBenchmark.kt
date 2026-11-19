package dev.yalin.benchmark

import androidx.benchmark.junit4.BenchmarkRule
import androidx.benchmark.junit4.measureRepeated
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.Parameterized

@RunWith(Parameterized::class)
class JsonBenchmark(private val payload: Payload, private val scenario: Scenario) {
  @get:Rule val rule = BenchmarkRule()

  @Test
  fun extract() {
    var sink: Extracted? = null
    rule.measureRepeated { sink = scenario.run(payload) }
    check(sink != null)
  }

  companion object {
    @JvmStatic
    @Parameterized.Parameters(name = "{0}_{1}")
    fun params(): List<Array<Any>> =
      Payloads.all.flatMap { p -> Scenario.entries.map { arrayOf<Any>(p, it) } }
  }
}
