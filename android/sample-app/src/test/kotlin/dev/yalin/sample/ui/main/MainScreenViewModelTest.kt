package dev.yalin.sample.ui.main

import androidx.lifecycle.viewModelScope
import dev.yalin.sample.data.DataRepository
import dev.yalin.sample.data.Dataset
import dev.yalin.sample.data.ParseResult
import dev.yalin.sample.data.ParserTime
import java.io.IOException
import junit.framework.TestCase.assertEquals
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.cancel
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class MainScreenViewModelTest {
  @Test
  fun measure_waitsForClickAndKeepsResultsWhenAnotherSourceFails() = runTest {
    Dispatchers.setMain(StandardTestDispatcher(testScheduler))
    val repository = FakeDataRepository()
    val viewModel = MainScreenViewModel(repository)
    try {
      val catalog = Dataset.CATALOG_16KB
      val cars = Dataset.VEGA_CARS
      assertEquals(MainScreenUiState(), viewModel.uiState.value)
      assertEquals(emptyList<Dataset>(), repository.requests)

      viewModel.measure(catalog)
      viewModel.measure(cars)
      assertEquals(MainScreenUiState(selected = catalog, isRunning = true), viewModel.uiState.value)
      runCurrent()
      assertEquals(listOf(catalog), repository.requests)
      assertEquals(mapOf(catalog to repository.result), viewModel.uiState.value.results)

      viewModel.measure(cars)
      runCurrent()
      assertEquals("Connection timed out", viewModel.uiState.value.error)
      assertEquals(mapOf(catalog to repository.result), viewModel.uiState.value.results)

      viewModel.measure(cars)
      runCurrent()
      assertEquals(listOf(catalog, cars, cars), repository.requests)
      assertEquals(
        MainScreenUiState(selected = cars, results = mapOf(catalog to repository.result, cars to repository.result)),
        viewModel.uiState.value,
      )
    } finally {
      viewModel.viewModelScope.cancel()
      runCurrent()
      Dispatchers.resetMain()
    }
  }
}

private class FakeDataRepository : DataRepository {
  val result = ParseResult(1_000, 6, listOf(ParserTime("Yalın", 50_000)))
  val requests = mutableListOf<Dataset>()

  override suspend fun measure(dataset: Dataset): ParseResult {
    requests += dataset
    if (requests == listOf(Dataset.CATALOG_16KB, Dataset.VEGA_CARS)) {
      throw IOException("Connection timed out")
    }
    return result
  }
}
